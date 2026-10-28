use std::ffi::CStr;
use std::ptr::{self, NonNull};

use ffmpeg_sys_next as sys;

use super::error::check;
use super::{Frame, Packet, Received};
use crate::MediaError;

pub(crate) const VIDEO_FORMAT: sys::AVPixelFormat = sys::AVPixelFormat::AV_PIX_FMT_YUV420P;
pub(crate) const AUDIO_FORMAT: sys::AVSampleFormat = sys::AVSampleFormat::AV_SAMPLE_FMT_FLTP;

pub(crate) struct Encoder(NonNull<sys::AVCodecContext>);

// SAFETY: the codec context is exclusively owned and only used through `&mut self`.
unsafe impl Send for Encoder {}

impl Encoder {
    /// H.264 in BT.709 4:2:0, through x264 when FFmpeg has it.
    pub(crate) fn video(
        (width, height): (u32, u32),
        (rate_num, rate_den): (u32, u32),
        crf: u8,
        global_header: bool,
    ) -> Result<Self, MediaError> {
        let invalid = || MediaError::InvalidSettings("frame size or rate out of range");
        // SAFETY: plain lookups returning static codec descriptions or null.
        let codec = unsafe {
            let x264 = sys::avcodec_find_encoder_by_name(c"libx264".as_ptr());
            if x264.is_null() {
                sys::avcodec_find_encoder(sys::AVCodecID::AV_CODEC_ID_H264)
            } else {
                x264
            }
        };
        let encoder = Self::alloc(codec)?;
        // SAFETY: the context was just allocated and is exclusively owned.
        let raw = unsafe { &mut *encoder.0.as_ptr() };
        raw.width = i32::try_from(width).map_err(|_| invalid())?;
        raw.height = i32::try_from(height).map_err(|_| invalid())?;
        let (num, den) = (
            i32::try_from(rate_num).map_err(|_| invalid())?,
            i32::try_from(rate_den).map_err(|_| invalid())?,
        );
        raw.framerate = sys::AVRational { num, den };
        raw.time_base = sys::AVRational { num: den, den: num };
        raw.sample_aspect_ratio = sys::AVRational { num: 1, den: 1 };
        raw.pix_fmt = VIDEO_FORMAT;
        raw.colorspace = sys::AVColorSpace::AVCOL_SPC_BT709;
        raw.color_primaries = sys::AVColorPrimaries::AVCOL_PRI_BT709;
        raw.color_trc = sys::AVColorTransferCharacteristic::AVCOL_TRC_BT709;
        raw.color_range = sys::AVColorRange::AVCOL_RANGE_MPEG;
        raw.thread_count = 0;
        let crf = std::ffi::CString::new(crf.to_string()).map_err(|_| invalid())?;
        encoder.open(
            codec,
            global_header,
            &[(c"crf", crf.as_c_str()), (c"preset", c"medium")],
        )
    }

    /// AAC stereo.
    pub(crate) fn audio(
        sample_rate: u32,
        bitrate: u32,
        global_header: bool,
    ) -> Result<Self, MediaError> {
        let invalid = || MediaError::InvalidSettings("sample rate out of range");
        // SAFETY: plain lookup returning a static codec description or null.
        let codec = unsafe { sys::avcodec_find_encoder(sys::AVCodecID::AV_CODEC_ID_AAC) };
        let encoder = Self::alloc(codec)?;
        // SAFETY: the context was just allocated and is exclusively owned; its default
        // channel layout owns no memory, so overwriting it leaks nothing.
        unsafe {
            let raw = &mut *encoder.0.as_ptr();
            raw.sample_rate = i32::try_from(sample_rate).map_err(|_| invalid())?;
            raw.time_base = sys::AVRational {
                num: 1,
                den: raw.sample_rate,
            };
            raw.sample_fmt = AUDIO_FORMAT;
            raw.bit_rate = i64::from(bitrate);
            sys::av_channel_layout_default(&mut raw.ch_layout, 2);
        }
        encoder.open(codec, global_header, &[])
    }

    fn alloc(codec: *const sys::AVCodec) -> Result<Self, MediaError> {
        if codec.is_null() {
            return Err(MediaError::NoEncoder);
        }
        // SAFETY: `codec` is a valid encoder; a null return means allocation failed.
        NonNull::new(unsafe { sys::avcodec_alloc_context3(codec) })
            .map(Self)
            .ok_or(MediaError::OutOfMemory)
    }

    /// Private options the codec doesn't know are ignored, so the same list can serve
    /// x264 and its fallbacks.
    fn open(
        self,
        codec: *const sys::AVCodec,
        global_header: bool,
        options: &[(&CStr, &CStr)],
    ) -> Result<Self, MediaError> {
        // SAFETY: the context is allocated for `codec` and exclusively owned; option
        // names and values are NUL-terminated strings that outlive the calls.
        unsafe {
            let raw = &mut *self.0.as_ptr();
            if global_header {
                raw.flags |= sys::AV_CODEC_FLAG_GLOBAL_HEADER as i32;
            }
            for (name, value) in options {
                sys::av_opt_set(raw.priv_data, name.as_ptr(), value.as_ptr(), 0);
            }
            check(sys::avcodec_open2(raw, codec, ptr::null_mut()))?;
        }
        Ok(self)
    }

    /// Sends `None` at the end so the encoder flushes its delayed packets.
    pub(crate) fn send_frame(&mut self, frame: Option<&Frame>) -> Result<(), MediaError> {
        let raw = frame.map_or(ptr::null(), |frame| ptr::from_ref(frame.raw()));
        // SAFETY: the context is open, and `raw` is null or a valid frame the caller borrows.
        check(unsafe { sys::avcodec_send_frame(self.0.as_ptr(), raw) }).map(drop)
    }

    pub(crate) fn receive_packet(&mut self, packet: &mut Packet) -> Result<Received, MediaError> {
        // SAFETY: the context is open and `packet` is a valid, exclusively borrowed packet.
        let code = unsafe { sys::avcodec_receive_packet(self.0.as_ptr(), packet.as_mut_ptr()) };
        match code {
            sys::AVERROR_EOF => Ok(Received::Drained),
            code if code == sys::AVERROR(sys::EAGAIN) => Ok(Received::NeedsInput),
            code => check(code).map(|_| Received::Frame),
        }
    }

    pub(super) fn raw(&self) -> &sys::AVCodecContext {
        // SAFETY: the context is valid for the lifetime of `self`.
        unsafe { self.0.as_ref() }
    }

    pub(crate) fn timebase(&self) -> sys::AVRational {
        self.raw().time_base
    }

    /// Samples per audio frame; `None` when the encoder takes any size.
    pub(crate) fn frame_size(&self) -> Option<usize> {
        usize::try_from(self.raw().frame_size)
            .ok()
            .filter(|&size| size > 0)
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        let mut raw = self.0.as_ptr();
        // SAFETY: the context came from avcodec_alloc_context3 and is freed exactly once here.
        unsafe { sys::avcodec_free_context(&mut raw) };
    }
}
