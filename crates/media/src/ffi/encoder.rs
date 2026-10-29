use std::ffi::{CStr, CString};
use std::ptr::{self, NonNull};

use ffmpeg_sys_next as sys;

use super::error::check;
use super::{Frame, Packet, Received};
use crate::{
    AudioCodec, AudioSettings, DnxhrQuality, EncodeProfile, MediaError, RateControl, VideoCodec,
    VideoSettings,
};

/// The smallest frame DNxHR encodes.
const DNXHR_SMALLEST: (u32, u32) = (256, 120);

pub(crate) struct Encoder(NonNull<sys::AVCodecContext>);

// SAFETY: the codec context is exclusively owned and only used through `&mut self`.
unsafe impl Send for Encoder {}

/// An encoder option, owned so values can be formatted.
type Opt = (&'static CStr, CString);

impl Encoder {
    /// BT.709, through x264, x265 or prores_ks when FFmpeg has them.
    pub(crate) fn video(settings: &VideoSettings, global_header: bool) -> Result<Self, MediaError> {
        let invalid = || MediaError::InvalidSettings("frame size or rate out of range");
        let (smallest_width, smallest_height) = DNXHR_SMALLEST;
        let too_small = settings.width < smallest_width || settings.height < smallest_height;
        if matches!(settings.codec, VideoCodec::Dnxhr(_)) && too_small {
            return Err(MediaError::InvalidSettings(
                "DNxHR needs a frame at least 256 × 120",
            ));
        }
        let codec = find_video(settings.codec);
        let encoder = Self::alloc(codec)?;
        // SAFETY: the context was just allocated and is exclusively owned.
        let raw = unsafe { &mut *encoder.0.as_ptr() };
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
        raw.pix_fmt = match settings.codec {
            VideoCodec::H264 | VideoCodec::Hevc => sys::AVPixelFormat::AV_PIX_FMT_YUV420P,
            VideoCodec::ProRes422 | VideoCodec::ProRes422Hq => {
                sys::AVPixelFormat::AV_PIX_FMT_YUV422P10LE
            }
            VideoCodec::ProRes4444 => sys::AVPixelFormat::AV_PIX_FMT_YUVA444P10LE,
            VideoCodec::Dnxhr(DnxhrQuality::Lb | DnxhrQuality::Sq | DnxhrQuality::Hq) => {
                sys::AVPixelFormat::AV_PIX_FMT_YUV422P
            }
            VideoCodec::Dnxhr(DnxhrQuality::Hqx) => sys::AVPixelFormat::AV_PIX_FMT_YUV422P10LE,
            VideoCodec::Dnxhr(DnxhrQuality::Dnx444) => sys::AVPixelFormat::AV_PIX_FMT_YUV444P10LE,
            VideoCodec::Gif => sys::AVPixelFormat::AV_PIX_FMT_PAL8,
        };
        raw.colorspace = sys::AVColorSpace::AVCOL_SPC_BT709;
        raw.color_primaries = sys::AVColorPrimaries::AVCOL_PRI_BT709;
        raw.color_trc = sys::AVColorTransferCharacteristic::AVCOL_TRC_BT709;
        raw.color_range = sys::AVColorRange::AVCOL_RANGE_MPEG;
        raw.thread_count = 0;
        let mut options: Vec<Opt> = Vec::new();
        if settings.codec.is_rate_controlled() {
            match settings.rate_control {
                RateControl::Quality(crf) => options.push((c"crf", number(crf)?)),
                RateControl::Bitrate(bits) => {
                    raw.bit_rate = i64::from(bits);
                    // Peaks may run half again over the average, smoothed over two seconds.
                    raw.rc_max_rate = raw.bit_rate * 3 / 2;
                    raw.rc_buffer_size = i32::try_from(raw.bit_rate * 2).unwrap_or(i32::MAX);
                }
            }
        }
        match (settings.codec, settings.profile) {
            (VideoCodec::H264, EncodeProfile::Delivery) => {
                options.push((c"preset", c"medium".to_owned()));
            }
            (VideoCodec::H264, EncodeProfile::Editing) => {
                raw.gop_size = (num / den / 2).max(1);
                raw.max_b_frames = 0;
                options.push((c"preset", c"veryfast".to_owned()));
                options.push((c"tune", c"fastdecode".to_owned()));
            }
            (VideoCodec::Hevc, _) => {
                options.push((c"preset", c"fast".to_owned()));
                options.push((c"x265-params", c"log-level=error".to_owned()));
            }
            (VideoCodec::ProRes422, _) => options.push((c"profile", c"standard".to_owned())),
            (VideoCodec::ProRes422Hq, _) => options.push((c"profile", c"hq".to_owned())),
            (VideoCodec::ProRes4444, _) => options.push((c"profile", c"4444".to_owned())),
            (VideoCodec::Dnxhr(quality), _) => {
                let profile = match quality {
                    DnxhrQuality::Lb => c"dnxhr_lb",
                    DnxhrQuality::Sq => c"dnxhr_sq",
                    DnxhrQuality::Hq => c"dnxhr_hq",
                    DnxhrQuality::Hqx => c"dnxhr_hqx",
                    DnxhrQuality::Dnx444 => c"dnxhr_444",
                };
                options.push((c"profile", profile.to_owned()));
            }
            // Both compare palette indices with the last picture's, but each picture has
            // its own palette, so an unchanged index can be a changed color.
            (VideoCodec::Gif, _) => {
                options.push((c"gifflags", c"-offsetting-transdiff".to_owned()))
            }
        }
        if settings.codec.is_prores() {
            // Apple's vendor code, which some players check before trusting the stream.
            options.push((c"vendor", c"apl0".to_owned()));
        }
        encoder.open(codec, global_header, &options)
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
        let raw = unsafe { &mut *encoder.0.as_ptr() };
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
            AudioCodec::Flac => (sys::AVCodecID::AV_CODEC_ID_FLAC, None),
            AudioCodec::Pcm16 => (sys::AVCodecID::AV_CODEC_ID_PCM_S16LE, None),
            AudioCodec::Pcm24 => (sys::AVCodecID::AV_CODEC_ID_PCM_S24LE, None),
        };
        let codec = find(name, id);
        let encoder = Self::alloc(codec)?;
        // SAFETY: the context was just allocated and is exclusively owned; its default
        // channel layout owns no memory, so overwriting it leaks nothing.
        unsafe {
            let raw = &mut *encoder.0.as_ptr();
            raw.sample_rate = i32::try_from(settings.sample_rate).map_err(|_| invalid())?;
            raw.time_base = sys::AVRational {
                num: 1,
                den: raw.sample_rate,
            };
            raw.sample_fmt = match settings.codec {
                AudioCodec::Aac | AudioCodec::Mp3 => sys::AVSampleFormat::AV_SAMPLE_FMT_FLTP,
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
        NonNull::new(unsafe { sys::avcodec_alloc_context3(codec) })
            .map(Self)
            .ok_or(MediaError::OutOfMemory)
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
        let mut raw = self.0.as_ptr();
        // SAFETY: the context came from avcodec_alloc_context3 and is freed exactly once here.
        unsafe { sys::avcodec_free_context(&mut raw) };
    }
}

fn find_video(codec: VideoCodec) -> *const sys::AVCodec {
    match codec {
        VideoCodec::H264 => find(Some(c"libx264"), sys::AVCodecID::AV_CODEC_ID_H264),
        VideoCodec::Hevc => find(Some(c"libx265"), sys::AVCodecID::AV_CODEC_ID_HEVC),
        VideoCodec::ProRes422 | VideoCodec::ProRes422Hq | VideoCodec::ProRes4444 => {
            find(Some(c"prores_ks"), sys::AVCodecID::AV_CODEC_ID_PRORES)
        }
        VideoCodec::Dnxhr(_) => find(None, sys::AVCodecID::AV_CODEC_ID_DNXHD),
        VideoCodec::Gif => find(None, sys::AVCodecID::AV_CODEC_ID_GIF),
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

fn number(value: impl ToString) -> Result<CString, MediaError> {
    CString::new(value.to_string()).map_err(|_| MediaError::InvalidSettings("option out of range"))
}
