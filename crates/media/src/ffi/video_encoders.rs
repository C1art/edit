//! Which FFmpeg encoders can make each video codec, and how each is told the settings,
//! since GPU and software encoders name the same choices differently.

use std::ffi::CStr;

use ffmpeg_sys_next as sys;

use super::encoder::{number, Opt};
use crate::{
    Acceleration, DnxhrQuality, EncodeProfile, MediaError, RateControl, VideoCodec, VideoSettings,
};

/// Bits a pixel x264 spends at CRF 18 on ordinary footage, for encoders that can only
/// be given a bitrate.
const BITS_PER_PIXEL_AT_18: f64 = 0.12;
/// HEVC's share of H.264's bitrate for the same look.
const HEVC_SHARE: f64 = 0.6;
const VP9_SHARE: f64 = 0.65;
const AV1_SHARE: f64 = 0.5;
/// The top of VP9's and AV1's quality scale, where H.264's is 51.
const WIDE_WORST: u16 = 63;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Family {
    Nvenc,
    Amf,
    QuickSync,
    X264,
    X265,
    Vpx,
    SvtAv1,
    Aom,
    /// Software encoders without a quality target, such as OpenH264 and Kvazaar.
    BitrateOnly,
    /// ProRes, DNxHR and GIF, which set their own data rates.
    Fixed,
}

impl Family {
    fn of(codec: VideoCodec, name: &CStr) -> Self {
        let name = name.to_bytes();
        if !codec.is_rate_controlled() {
            Self::Fixed
        } else if name.ends_with(b"_nvenc") {
            Self::Nvenc
        } else if name.ends_with(b"_amf") {
            Self::Amf
        } else if name.ends_with(b"_qsv") {
            Self::QuickSync
        } else if name == b"libx264" {
            Self::X264
        } else if name == b"libx265" {
            Self::X265
        } else if name == b"libvpx-vp9" {
            Self::Vpx
        } else if name == b"libsvtav1" {
            Self::SvtAv1
        } else if name == b"libaom-av1" {
            Self::Aom
        } else {
            Self::BitrateOnly
        }
    }

    fn is_gpu(self) -> bool {
        matches!(self, Self::Nvenc | Self::Amf | Self::QuickSync)
    }

    /// What the user is told encoded their export.
    pub(super) fn describe(self, codec_name: &CStr) -> String {
        match self {
            Self::Nvenc => "NVIDIA NVENC (hardware)".to_owned(),
            Self::Amf => "AMD AMF (hardware)".to_owned(),
            Self::QuickSync => "Intel Quick Sync (hardware)".to_owned(),
            _ => format!("{} (software)", codec_name.to_string_lossy()),
        }
    }
}

/// An encoder FFmpeg has for the codec, to be tried in turn: one found may still fail
/// to open when its GPU or driver is missing.
pub(super) struct Candidate {
    pub(super) codec: *const sys::AVCodec,
    pub(super) family: Family,
}

/// GPU encoders first when allowed, NVENC ahead of AMF ahead of Quick Sync, then
/// software, best first, then whatever FFmpeg picks for the codec.
pub(super) fn candidates(codec: VideoCodec, acceleration: Acceleration) -> Vec<Candidate> {
    let (names, id): (&[&CStr], _) = match codec {
        VideoCodec::H264 => (
            &[
                c"h264_nvenc",
                c"h264_amf",
                c"h264_qsv",
                c"libx264",
                c"libopenh264",
            ],
            sys::AVCodecID::AV_CODEC_ID_H264,
        ),
        VideoCodec::Hevc => (
            &[
                c"hevc_nvenc",
                c"hevc_amf",
                c"hevc_qsv",
                c"libx265",
                c"libkvazaar",
            ],
            sys::AVCodecID::AV_CODEC_ID_HEVC,
        ),
        VideoCodec::Vp9 => (&[c"libvpx-vp9"], sys::AVCodecID::AV_CODEC_ID_VP9),
        VideoCodec::Av1 => (
            &[
                c"av1_nvenc",
                c"av1_amf",
                c"av1_qsv",
                c"libsvtav1",
                c"libaom-av1",
                c"librav1e",
            ],
            sys::AVCodecID::AV_CODEC_ID_AV1,
        ),
        VideoCodec::ProRes422 | VideoCodec::ProRes422Hq | VideoCodec::ProRes4444 => {
            (&[c"prores_ks"], sys::AVCodecID::AV_CODEC_ID_PRORES)
        }
        VideoCodec::Dnxhr(_) => (&[], sys::AVCodecID::AV_CODEC_ID_DNXHD),
        VideoCodec::Gif => (&[], sys::AVCodecID::AV_CODEC_ID_GIF),
    };
    // SAFETY: plain lookups returning static codec descriptions or null.
    let named = names
        .iter()
        .map(|name| unsafe { sys::avcodec_find_encoder_by_name(name.as_ptr()) });
    // SAFETY: as above.
    let default = unsafe { sys::avcodec_find_encoder(id) };
    let mut found: Vec<Candidate> = Vec::new();
    for codec_ptr in named.chain([default]).filter(|codec| !codec.is_null()) {
        // SAFETY: a non-null encoder description is static, with a NUL-terminated name.
        let name = unsafe { CStr::from_ptr((*codec_ptr).name) };
        let family = Family::of(codec, name);
        let unwanted = acceleration == Acceleration::Software && family.is_gpu();
        if !unwanted && !found.iter().any(|seen| seen.codec == codec_ptr) {
            found.push(Candidate {
                codec: codec_ptr,
                family,
            });
        }
    }
    found
}

/// GPU encoders take NV12, which all three read natively.
pub(super) fn pixel_format(codec: VideoCodec, family: Family) -> sys::AVPixelFormat {
    use sys::AVPixelFormat as Format;
    match codec {
        _ if family.is_gpu() => Format::AV_PIX_FMT_NV12,
        VideoCodec::H264 | VideoCodec::Hevc | VideoCodec::Vp9 | VideoCodec::Av1 => {
            Format::AV_PIX_FMT_YUV420P
        }
        VideoCodec::ProRes422 | VideoCodec::ProRes422Hq => Format::AV_PIX_FMT_YUV422P10LE,
        VideoCodec::ProRes4444 => Format::AV_PIX_FMT_YUVA444P10LE,
        VideoCodec::Dnxhr(DnxhrQuality::Lb | DnxhrQuality::Sq | DnxhrQuality::Hq) => {
            Format::AV_PIX_FMT_YUV422P
        }
        VideoCodec::Dnxhr(DnxhrQuality::Hqx) => Format::AV_PIX_FMT_YUV422P10LE,
        VideoCodec::Dnxhr(DnxhrQuality::Dnx444) => Format::AV_PIX_FMT_YUV444P10LE,
        VideoCodec::Gif => Format::AV_PIX_FMT_PAL8,
    }
}

/// Sets the rate, speed and keyframe choices on a context allocated for an encoder of
/// `family`, returning the private options to open it with.
pub(super) fn configure(
    raw: &mut sys::AVCodecContext,
    family: Family,
    settings: &VideoSettings,
) -> Result<Vec<Opt>, MediaError> {
    let mut options: Vec<Opt> = Vec::new();
    let editing = settings.profile == EncodeProfile::Editing;
    if family != Family::Fixed {
        rate(raw, family, settings, &mut options)?;
        if editing {
            raw.gop_size = (raw.framerate.num / raw.framerate.den.max(1) / 2).max(1);
            raw.max_b_frames = 0;
        }
    }
    let option = |name: &'static CStr, value: &CStr| (name, value.to_owned());
    match (family, settings.codec) {
        (Family::X264, _) if editing => {
            options.push(option(c"preset", c"veryfast"));
            options.push(option(c"tune", c"fastdecode"));
        }
        (Family::X264, _) => options.push(option(c"preset", c"medium")),
        (Family::X265, _) => {
            options.push(option(c"preset", c"fast"));
            options.push(option(c"x265-params", c"log-level=error"));
        }
        (Family::Nvenc, _) => {
            options.push(option(c"preset", if editing { c"p2" } else { c"p5" }));
            options.push(option(c"spatial-aq", c"1"));
        }
        (Family::Amf, _) => {
            options.push(option(c"usage", c"transcoding"));
            options.push(option(
                c"quality",
                if editing { c"speed" } else { c"quality" },
            ));
        }
        (Family::QuickSync, _) => {
            options.push(option(
                c"preset",
                if editing { c"veryfast" } else { c"medium" },
            ));
        }
        (Family::Vpx, _) => {
            options.push(option(c"deadline", c"good"));
            options.push(option(c"cpu-used", if editing { c"5" } else { c"3" }));
            options.push(option(c"row-mt", c"1"));
        }
        (Family::SvtAv1, _) => {
            options.push(option(c"preset", if editing { c"12" } else { c"8" }));
        }
        (Family::Aom, _) => {
            options.push(option(c"cpu-used", if editing { c"8" } else { c"6" }));
            options.push(option(c"row-mt", c"1"));
        }
        (Family::BitrateOnly, _) => {}
        (Family::Fixed, VideoCodec::ProRes422) => options.push(option(c"profile", c"standard")),
        (Family::Fixed, VideoCodec::ProRes422Hq) => options.push(option(c"profile", c"hq")),
        (Family::Fixed, VideoCodec::ProRes4444) => options.push(option(c"profile", c"4444")),
        (Family::Fixed, VideoCodec::Dnxhr(quality)) => {
            let profile = match quality {
                DnxhrQuality::Lb => c"dnxhr_lb",
                DnxhrQuality::Sq => c"dnxhr_sq",
                DnxhrQuality::Hq => c"dnxhr_hq",
                DnxhrQuality::Hqx => c"dnxhr_hqx",
                DnxhrQuality::Dnx444 => c"dnxhr_444",
            };
            options.push(option(c"profile", profile));
        }
        // Both compare palette indices with the last picture's, but each picture has
        // its own palette, so an unchanged index can be a changed color.
        (Family::Fixed, VideoCodec::Gif) => {
            options.push(option(c"gifflags", c"-offsetting-transdiff"));
        }
        (
            Family::Fixed,
            VideoCodec::H264 | VideoCodec::Hevc | VideoCodec::Vp9 | VideoCodec::Av1,
        ) => {}
    }
    if settings.codec.is_prores() {
        // Apple's vendor code, which some players check before trusting the stream.
        options.push(option(c"vendor", c"apl0"));
    }
    Ok(options)
}

fn rate(
    raw: &mut sys::AVCodecContext,
    family: Family,
    settings: &VideoSettings,
    options: &mut Vec<Opt>,
) -> Result<(), MediaError> {
    let quality = match settings.rate_control {
        RateControl::Quality(quality) if family == Family::BitrateOnly => {
            set_bitrate(raw, quality_bitrate(settings, quality));
            return Ok(());
        }
        RateControl::Quality(quality) => quality,
        RateControl::Bitrate(bits) => {
            set_bitrate(raw, i64::from(bits));
            match family {
                Family::Nvenc => options.push((c"rc", c"vbr".to_owned())),
                Family::Amf => options.push((c"rc", c"vbr_peak".to_owned())),
                _ => {}
            }
            return Ok(());
        }
    };
    // GPU encoders come with a default bitrate that would otherwise cap the quality,
    // and libvpx targets constant quality only without one.
    raw.bit_rate = 0;
    let quality = native_quality(settings.codec, family, quality);
    let quality_text = number(quality)?;
    match family {
        Family::X264 | Family::X265 | Family::Vpx | Family::SvtAv1 | Family::Aom => {
            options.push((c"crf", quality_text));
        }
        Family::Nvenc => {
            options.push((c"rc", c"vbr".to_owned()));
            options.push((c"cq", quality_text));
        }
        // Constant QP, which every AMF and Quick Sync generation supports, unlike their
        // quality-targeting modes.
        Family::Amf => {
            options.push((c"rc", c"cqp".to_owned()));
            for name in [c"qp_i", c"qp_p", c"qp_b"] {
                options.push((name, quality_text.clone()));
            }
        }
        Family::QuickSync => {
            raw.flags |= sys::AV_CODEC_FLAG_QSCALE as i32;
            raw.global_quality = i32::from(quality) * sys::FF_QP2LAMBDA;
        }
        Family::BitrateOnly | Family::Fixed => {}
    }
    Ok(())
}

/// Settings give quality on x264's CRF scale; VP9 and AV1 count to 63 instead, and
/// AV1's AMF and Quick Sync quantizers to 255.
fn native_quality(codec: VideoCodec, family: Family, quality: u8) -> u16 {
    let wide = (u16::from(quality) * 3 / 2 + 1).min(WIDE_WORST);
    match (codec, family) {
        (VideoCodec::Av1, Family::Amf | Family::QuickSync) => wide * 4,
        (_, Family::Vpx | Family::SvtAv1 | Family::Aom) => wide,
        _ => u16::from(quality),
    }
}

/// Peaks may run half again over the average, smoothed over two seconds.
fn set_bitrate(raw: &mut sys::AVCodecContext, bits: i64) {
    raw.bit_rate = bits;
    raw.rc_max_rate = bits * 3 / 2;
    raw.rc_buffer_size = i32::try_from(bits * 2).unwrap_or(i32::MAX);
}

/// About what x264 spends at CRF `quality` on these settings: each 6 steps halves or
/// doubles the rate, as CRF does.
fn quality_bitrate(settings: &VideoSettings, quality: u8) -> i64 {
    let frames_per_second = settings.frame_rate.approx_fps();
    let pixels = f64::from(settings.width) * f64::from(settings.height) * frames_per_second;
    let share = match settings.codec {
        VideoCodec::Hevc => HEVC_SHARE,
        VideoCodec::Vp9 => VP9_SHARE,
        VideoCodec::Av1 => AV1_SHARE,
        _ => 1.0,
    };
    let bits_per_pixel = BITS_PER_PIXEL_AT_18 * ((18.0 - f64::from(quality)) / 6.0).exp2();
    (pixels * bits_per_pixel * share).round() as i64
}

#[cfg(test)]
mod tests {
    use timeline::FrameRate;

    use super::*;

    #[test]
    fn a_quality_comes_to_a_bitrate_that_halves_every_six_steps() {
        let settings = |codec, quality| VideoSettings {
            input_size: (1920, 1080),
            width: 1920,
            height: 1080,
            frame_rate: FrameRate::FPS_25,
            codec,
            rate_control: RateControl::Quality(quality),
            profile: EncodeProfile::Delivery,
            acceleration: Acceleration::Software,
        };
        let at = |codec, quality| quality_bitrate(&settings(codec, quality), quality);
        assert_eq!(at(VideoCodec::H264, 18), 6_220_800);
        assert_eq!(at(VideoCodec::H264, 24), 3_110_400);
        assert_eq!(at(VideoCodec::Hevc, 18), 3_732_480);
    }

    #[test]
    fn software_leaves_out_gpu_encoders() {
        let gpu = |acceleration| {
            candidates(VideoCodec::H264, acceleration)
                .iter()
                .any(|candidate| candidate.family.is_gpu())
        };
        assert!(!gpu(Acceleration::Software));
    }
}
