use media::{
    Acceleration, AudioCodec, AudioSettings, DnxhrQuality, RateControl, StillFormat, VideoCodec,
};
use playback::{VideoEncoding, VideoExport};

/// What kind of file an export makes: its codecs and container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    H264,
    Hevc,
    ProRes422,
    ProRes422Hq,
    ProRes4444,
    Dnxhr(DnxhrQuality),
    AnimatedGif,
    /// Each frame as a numbered image.
    Stills(StillFormat),
    Aac,
    Mp3,
    Wav,
    Flac,
}

impl Format {
    pub const ALL: [Self; 18] = [
        Self::H264,
        Self::Hevc,
        Self::ProRes422,
        Self::ProRes422Hq,
        Self::ProRes4444,
        Self::Dnxhr(DnxhrQuality::Lb),
        Self::Dnxhr(DnxhrQuality::Sq),
        Self::Dnxhr(DnxhrQuality::Hq),
        Self::Dnxhr(DnxhrQuality::Hqx),
        Self::Dnxhr(DnxhrQuality::Dnx444),
        Self::AnimatedGif,
        Self::Stills(StillFormat::Png),
        Self::Stills(StillFormat::Tiff),
        Self::Stills(StillFormat::Dpx),
        Self::Aac,
        Self::Mp3,
        Self::Wav,
        Self::Flac,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::H264 => "H.264",
            Self::Hevc => "HEVC (H.265)",
            Self::ProRes422 => "Apple ProRes 422",
            Self::ProRes422Hq => "Apple ProRes 422 HQ",
            Self::ProRes4444 => "Apple ProRes 4444 with Alpha",
            Self::Dnxhr(DnxhrQuality::Lb) => "Avid DNxHR LB",
            Self::Dnxhr(DnxhrQuality::Sq) => "Avid DNxHR SQ",
            Self::Dnxhr(DnxhrQuality::Hq) => "Avid DNxHR HQ",
            Self::Dnxhr(DnxhrQuality::Hqx) => "Avid DNxHR HQX 10-bit",
            Self::Dnxhr(DnxhrQuality::Dnx444) => "Avid DNxHR 444 10-bit",
            Self::AnimatedGif => "Animated GIF",
            Self::Stills(StillFormat::Png) => "PNG Sequence",
            Self::Stills(StillFormat::Tiff) => "TIFF Sequence",
            Self::Stills(StillFormat::Dpx) => "DPX Sequence",
            Self::Aac => "AAC Audio",
            Self::Mp3 => "MP3",
            Self::Wav => "Waveform Audio",
            Self::Flac => "FLAC",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::H264 | Self::Hevc => "mp4",
            Self::ProRes422 | Self::ProRes422Hq | Self::ProRes4444 | Self::Dnxhr(_) => "mov",
            Self::AnimatedGif => "gif",
            Self::Stills(still) => still.extension(),
            Self::Aac => "m4a",
            Self::Mp3 => "mp3",
            Self::Wav => "wav",
            Self::Flac => "flac",
        }
    }

    pub fn video_codec(self) -> Option<VideoCodec> {
        match self {
            Self::H264 => Some(VideoCodec::H264),
            Self::Hevc => Some(VideoCodec::Hevc),
            Self::ProRes422 => Some(VideoCodec::ProRes422),
            Self::ProRes422Hq => Some(VideoCodec::ProRes422Hq),
            Self::ProRes4444 => Some(VideoCodec::ProRes4444),
            Self::Dnxhr(quality) => Some(VideoCodec::Dnxhr(quality)),
            Self::AnimatedGif => Some(VideoCodec::Gif),
            Self::Stills(_) | Self::Aac | Self::Mp3 | Self::Wav | Self::Flac => None,
        }
    }

    pub fn has_video(self) -> bool {
        self.video_codec().is_some() || matches!(self, Self::Stills(_))
    }

    /// `None` for formats that can't carry sound.
    pub fn audio_codec(self) -> Option<AudioCodec> {
        match self {
            Self::H264 | Self::Hevc | Self::Aac => Some(AudioCodec::Aac),
            // Mastering formats carry uncompressed sound, as cameras and Premiere do.
            Self::ProRes422 | Self::ProRes422Hq | Self::ProRes4444 | Self::Dnxhr(_) => {
                Some(AudioCodec::Pcm24)
            }
            Self::AnimatedGif | Self::Stills(_) => None,
            Self::Mp3 => Some(AudioCodec::Mp3),
            Self::Wav => Some(AudioCodec::Pcm16),
            Self::Flac => Some(AudioCodec::Flac),
        }
    }
}

/// Everything an export writes, apart from where and what range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportFormat {
    pub format: Format,
    /// The output frame height, the width following the sequence's shape; the
    /// sequence's own size when `None`.
    pub height: Option<u32>,
    pub rate_control: RateControl,
    /// Kept when a preset is picked, as it says how to encode rather than what.
    pub acceleration: Acceleration,
    /// Whether a video export carries sound; audio formats always do.
    pub with_audio: bool,
    pub audio_kbps: u32,
    pub sample_rate: u32,
}

pub struct Preset {
    pub name: &'static str,
    pub format: ExportFormat,
    /// Made for a vertical sequence, as phone apps show it.
    pub vertical: bool,
}

const MATCH_SOURCE: ExportFormat = ExportFormat {
    format: Format::H264,
    height: None,
    rate_control: RateControl::Quality(18),
    acceleration: Acceleration::Hardware,
    with_audio: true,
    audio_kbps: 320,
    sample_rate: 48_000,
};

const fn video_preset(name: &'static str, height: u32, mbps: u32) -> Preset {
    Preset {
        name,
        format: ExportFormat {
            height: Some(height),
            rate_control: RateControl::Bitrate(mbps * 1_000_000),
            ..MATCH_SOURCE
        },
        vertical: false,
    }
}

/// 1080 × 1920 from a vertical sequence.
const fn vertical_preset(name: &'static str, mbps: u32) -> Preset {
    Preset {
        vertical: true,
        ..video_preset(name, 1920, mbps)
    }
}

const fn format_preset(name: &'static str, format: Format) -> Preset {
    Preset {
        name,
        format: ExportFormat {
            format,
            ..MATCH_SOURCE
        },
        vertical: false,
    }
}

pub const PRESETS: [Preset; 18] = [
    Preset {
        name: "Match Source - High Bitrate",
        format: MATCH_SOURCE,
        vertical: false,
    },
    video_preset("YouTube 2160p 4K Ultra HD", 2160, 45),
    video_preset("YouTube 1080p Full HD", 1080, 16),
    video_preset("Vimeo 720p HD", 720, 10),
    vertical_preset("YouTube Shorts 1080×1920", 16),
    vertical_preset("TikTok 1080×1920", 12),
    vertical_preset("Instagram Reels 1080×1920", 10),
    Preset {
        name: "HEVC Match Source",
        format: ExportFormat {
            format: Format::Hevc,
            rate_control: RateControl::Quality(22),
            ..MATCH_SOURCE
        },
        vertical: false,
    },
    format_preset("Apple ProRes 422", Format::ProRes422),
    format_preset("Apple ProRes 422 HQ", Format::ProRes422Hq),
    format_preset("Apple ProRes 4444 with Alpha", Format::ProRes4444),
    format_preset("DNxHR HQ", Format::Dnxhr(DnxhrQuality::Hq)),
    format_preset("DNxHR HQX 10-bit", Format::Dnxhr(DnxhrQuality::Hqx)),
    Preset {
        name: "Animated GIF 480p",
        format: ExportFormat {
            format: Format::AnimatedGif,
            height: Some(480),
            ..MATCH_SOURCE
        },
        vertical: false,
    },
    format_preset("AAC Audio 320 kbps", Format::Aac),
    format_preset("MP3 320 kbps", Format::Mp3),
    format_preset("Waveform Audio 48 kHz 16-bit", Format::Wav),
    format_preset("FLAC Lossless", Format::Flac),
];

impl Default for ExportFormat {
    fn default() -> Self {
        MATCH_SOURCE
    }
}

impl ExportFormat {
    /// The preset these settings match, if any, ignoring choices the format doesn't use.
    pub fn preset(&self) -> Option<&'static Preset> {
        let used = self.used();
        PRESETS.iter().find(|preset| preset.format.used() == used)
    }

    pub fn preset_name(&self) -> Option<&'static str> {
        self.preset().map(|preset| preset.name)
    }

    /// These settings with choices the format ignores put back to their defaults.
    fn used(self) -> Self {
        let unused = MATCH_SOURCE;
        let codec = self.format.video_codec();
        let rate_controlled = codec.is_some_and(VideoCodec::is_rate_controlled);
        Self {
            height: if self.format.has_video() {
                self.height
            } else {
                unused.height
            },
            rate_control: if rate_controlled {
                self.rate_control
            } else {
                unused.rate_control
            },
            acceleration: unused.acceleration,
            with_audio: !self.format.has_video() || self.with_audio,
            audio_kbps: if self.has_audio() && self.audio_is_lossy() {
                self.audio_kbps
            } else {
                unused.audio_kbps
            },
            sample_rate: if self.has_audio() {
                self.sample_rate
            } else {
                unused.sample_rate
            },
            ..self
        }
    }

    pub fn has_audio(&self) -> bool {
        let wanted = self.with_audio || !self.format.has_video();
        wanted && self.format.audio_codec().is_some()
    }

    pub fn audio_is_lossy(&self) -> bool {
        self.format.audio_codec().is_some_and(AudioCodec::is_lossy)
    }

    pub fn video(&self, sequence_size: (u32, u32)) -> Option<VideoExport> {
        let encoding = match self.format {
            Format::Stills(still) => VideoEncoding::Stills(still),
            format => VideoEncoding::Movie(format.video_codec()?, self.rate_control),
        };
        Some(VideoExport {
            encoding,
            size: self.size(sequence_size),
            acceleration: self.acceleration,
        })
    }

    pub fn size(&self, sequence_size: (u32, u32)) -> (u32, u32) {
        self.height
            .map_or(sequence_size, |height| fit_height(sequence_size, height))
    }

    pub fn audio(&self) -> Option<AudioSettings> {
        let codec = self.format.audio_codec().filter(|_| self.has_audio())?;
        Some(AudioSettings {
            codec,
            sample_rate: self.sample_rate,
            bits_per_second: self.audio_kbps * 1_000,
        })
    }
}

/// A frame of `(width, height)` scaled to `height`, keeping its shape, with an even
/// width as 4:2:0 chroma needs.
pub fn fit_height((width, source_height): (u32, u32), height: u32) -> (u32, u32) {
    let (width, source_height) = (u64::from(width), u64::from(source_height).max(1));
    let pairs = (width * u64::from(height) + source_height) / (2 * source_height);
    (u32::try_from(pairs * 2).unwrap_or(u32::MAX), height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fitting_a_height_keeps_the_shape_with_an_even_width() {
        assert_eq!(fit_height((1920, 1080), 720), (1280, 720));
        assert_eq!(fit_height((1280, 720), 480), (854, 480));
        assert_eq!(fit_height((1080, 1920), 1280), (720, 1280));
    }

    #[test]
    fn presets_match_ignoring_choices_the_format_does_not_use() {
        let leftover = ExportFormat {
            format: Format::Wav,
            height: Some(360),
            rate_control: RateControl::Bitrate(1_000_000),
            audio_kbps: 128,
            ..ExportFormat::default()
        };
        assert_eq!(leftover.preset_name(), Some("Waveform Audio 48 kHz 16-bit"));
        let resampled = ExportFormat {
            sample_rate: 44_100,
            ..leftover
        };
        assert_eq!(resampled.preset_name(), None);
    }
}
