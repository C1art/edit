use std::ffi::{CStr, CString};
use std::ptr::{self, NonNull};

use ffmpeg_sys_next as sys;

use super::error::check;
use super::video_encoders::{self, Family};
use super::{Frame, Packet, Received};
use crate::{AudioCodec, AudioSettings, MediaError, VideoCodec, VideoSettings};

/// The smallest frame DNxHR encodes.
const DNXHR_SMALLEST: (u32, u32) = (256, 120);

pub(crate) struct Encoder {
    context: NonNull<sys::AVCodecContext>,
    family: Family,
}

// SAFETY: the codec context is exclusively owned and only used through `&mut self`.
unsafe impl Send for Encoder {}

/// An encoder option, owned so values can be formatted.
pub(super) type Opt = (&'static CStr, CString);

impl Encoder {
    /// BT.709, through the first encoder for the codec that opens: a GPU encoder when
    /// `settings` allow one, otherwise software.
    pub(crate) fn video(settings: &VideoSettings, global_header: bool) -> Result<Self, MediaError> {
        let (smallest_width, smallest_height) = DNXHR_SMALLEST;
        let too_small = settings.width < smallest_width || settings.height < smallest_height;
        if matches!(settings.codec, VideoCodec::Dnxhr(_)) && too_small {
            return Err(MediaError::InvalidSettings(
                "DNxHR needs a frame at least 256 × 120",
            ));
        }
        let mut failure = MediaError::NoEncoder;
        for candidate in video_encoders::candidates(settings.codec, settings.acceleration) {
            let opened = Self::alloc(candidate.codec).and_then(|encoder| {
                let options = encoder.set_video(settings, candidate.family)?;
                encoder.open(candidate.codec, global_header, &options)
            });
            match opened {
                Ok(mut encoder) => {
                    encoder.family = candidate.family;
                    return Ok(encoder);
                }
                Err(error) => failure = error,
            }
        }
        Err(failure)
    }

    fn set_video(&self, settings: &VideoSettings, family: Family) -> Result<Vec<Opt>, MediaError> {
        let invalid = || MediaError::InvalidSettings("frame size or rate out of range");
        // SAFETY: the context was just allocated and is exclusively owned.
        let raw = unsafe { &mut *self.context.as_ptr() };
        raw.width = i32::try_from(settings.width).map_err(|_| invalid())?;
        raw.height = i32::try_from(settings.height).map_err(|_| invalid())?;
        let rate = settings.frame_rate;
        let (num, den) = (
            i32::try_from(rate.num()).map_err(|_| invalid())?,
            i32::try_from(rate.den()).map_err(|_| invalid())?,
        );
        raw.framerate = sys::AVRational { num, den };
        raw.time_base = sys::AVRational { num: den, den: num };
        raw.sample_aspect_ratio = sys::AVRational { num: 1, den: 1 };
        raw.pix_fmt = video_encoders::pixel_format(settings.codec, family);
        raw.colorspace = sys::AVColorSpace::AVCOL_SPC_BT709;
        raw.color_primaries = sys::AVColorPrimaries::AVCOL_PRI_BT709;
        raw.color_trc = sys::AVColorTransferCharacteristic::AVCOL_TRC_BT709;
        raw.color_range = sys::AVColorRange::AVCOL_RANGE_MPEG;
        raw.thread_count = 0;
        video_encoders::configure(raw, family, settings)
    }

    /// What encoded the video, to tell the user, such as "NVIDIA NVENC (hardware)".
    pub(crate) fn describe(&self) -> String {
        // SAFETY: an open context points at its static codec description, whose name
        // is a NUL-terminated string.
        let name = unsafe { CStr::from_ptr((*self.raw().codec).name) };
        self.family.describe(name)
    }

    /// One RGBA picture through an image codec, such as PNG, whose single packet is the
    /// whole image file.
    pub(crate) fn still(
        id: sys::AVCodecID,
        (width, height): (u32, u32),
    ) -> Result<Self, MediaError> {
        let invalid = || MediaError::InvalidSettings("frame size out of range");
        let codec = find(None, id);
        let encoder = Self::alloc(codec)?;
        // SAFETY: the context was just allocated and is exclusively owned.
        let raw = unsafe { &mut *encoder.context.as_ptr() };
        raw.width = i32::try_from(width).map_err(|_| invalid())?;
        raw.height = i32::try_from(height).map_err(|_| invalid())?;
        raw.time_base = sys::AVRational { num: 1, den: 1 };
        raw.pix_fmt = sys::AVPixelFormat::AV_PIX_FMT_RGBA;
        encoder.open(codec, false, &[])
    }

    /// Stereo.
    pub(crate) fn audio(settings: &AudioSettings, global_header: bool) -> Result<Self, MediaError> {
        let invalid = || MediaError::InvalidSettings("sample rate out of range");
        let (id, name) = match settings.codec {
            AudioCodec::Aac => (sys::AVCodecID::AV_CODEC_ID_AAC, None),
            AudioCodec::Mp3 => (sys::AVCodecID::AV_CODEC_ID_MP3, Some(c"libmp3lame")),
            AudioCodec::Opus => (sys::AVCodecID::AV_CODEC_ID_OPUS, Some(c"libopus")),
            AudioCodec::Flac => (sys::AVCodecID::AV_CODEC_ID_FLAC, None),
            AudioCodec::Pcm16 => (sys::AVCodecID::AV_CODEC_ID_PCM_S16LE, None),
            AudioCodec::Pcm24 => (sys::AVCodecID::AV_CODEC_ID_PCM_S24LE, None),
        };
        let codec = find(name, id);
        let encoder = Self::alloc(codec)?;
        // SAFETY: the context was just allocated and is exclusively owned; its default
        // channel layout owns no memory, so overwriting it leaks nothing.
        unsafe {
            let raw = &mut *encoder.context.as_ptr();
            raw.sample_rate = i32::try_from(settings.sample_rate).map_err(|_| invalid())?;
            raw.time_base = sys::AVRational {
                num: 1,
                den: raw.sample_rate,
            };
            raw.sample_fmt = match settings.codec {
                AudioCodec::Aac | AudioCodec::Mp3 => sys::AVSampleFormat::AV_SAMPLE_FMT_FLTP,
                AudioCodec::Opus => sys::AVSampleFormat::AV_SAMPLE_FMT_FLT,
                AudioCodec::Flac | AudioCodec::Pcm16 => sys::AVSampleFormat::AV_SAMPLE_FMT_S16,
                AudioCodec::Pcm24 => sys::AVSampleFormat::AV_SAMPLE_FMT_S32,
            };
            if settings.codec.is_lossy() {
                raw.bit_rate = i64::from(settings.bits_per_second);
            }
            sys::av_channel_layout_default(&mut raw.ch_layout, 2);
        }
        encoder.open(codec, global_header, &[])
    }

    fn alloc(codec: *const sys::AVCodec) -> Result<Self, MediaError> {
        if codec.is_null() {
            return Err(MediaError::NoEncoder);
        }
        // SAFETY: `codec` is a valid encoder; a null return means allocation failed.
        let context = NonNull::new(unsafe { sys::avcodec_alloc_context3(codec) })
            .ok_or(MediaError::OutOfMemory)?;
        Ok(Self {
            context,
            family: Family::Fixed,
        })
    }

    /// Private options the codec doesn't know are ignored, so the same list can serve
    /// an encoder and its fallbacks.
    fn open(
        self,
        codec: *const sys::AVCodec,
        global_header: bool,
        options: &[Opt],
    ) -> Result<Self, MediaError> {
        // SAFETY: the context is allocated for `codec` and exclusively owned; option
        // names and values are NUL-terminated strings that outlive the calls.
        unsafe {
            let raw = &mut *self.context.as_ptr();
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
        check(unsafe { sys::avcodec_send_frame(self.context.as_ptr(), raw) }).map(drop)
    }

    pub(crate) fn receive_packet(&mut self, packet: &mut Packet) -> Result<Received, MediaError> {
        // SAFETY: the context is open and `packet` is a valid, exclusively borrowed packet.
        let code =
            unsafe { sys::avcodec_receive_packet(self.context.as_ptr(), packet.as_mut_ptr()) };
        match code {
            sys::AVERROR_EOF => Ok(Received::Drained),
            code if code == sys::AVERROR(sys::EAGAIN) => Ok(Received::NeedsInput),
            code => check(code).map(|_| Received::Frame),
        }
    }

    pub(super) fn raw(&self) -> &sys::AVCodecContext {
        // SAFETY: the context is valid for the lifetime of `self`.
        unsafe { self.context.as_ref() }
    }

    pub(super) fn timebase(&self) -> sys::AVRational {
        self.raw().time_base
    }

    pub(super) fn pixel_format(&self) -> sys::AVPixelFormat {
        self.raw().pix_fmt
    }

    /// A blank picture of `size` in the format this encoder takes.
    pub(crate) fn video_frame(&self, size: (u32, u32)) -> Result<Frame, MediaError> {
        Frame::for_video(size, self.pixel_format())
    }

    /// A blank block of `samples` in the format this encoder takes.
    pub(crate) fn audio_frame(
        &self,
        sample_rate: u32,
        samples: usize,
    ) -> Result<Frame, MediaError> {
        Frame::for_audio(sample_rate, samples, self.raw().sample_fmt)
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
        let mut raw = self.context.as_ptr();
        // SAFETY: the context came from avcodec_alloc_context3 and is freed exactly once here.
        unsafe { sys::avcodec_free_context(&mut raw) };
    }
}

/// The encoder called `name`, or FFmpeg's default for `id` when it lacks that one.
fn find(name: Option<&CStr>, id: sys::AVCodecID) -> *const sys::AVCodec {
    // SAFETY: plain lookups returning static codec descriptions or null.
    unsafe {
        name.map(|name| sys::avcodec_find_encoder_by_name(name.as_ptr()))
            .filter(|codec| !codec.is_null())
            .unwrap_or_else(|| sys::avcodec_find_encoder(id))
    }
}

pub(super) fn number(value: impl ToString) -> Result<CString, MediaError> {
    CString::new(value.to_string()).map_err(|_| MediaError::InvalidSettings("option out of range"))
}
