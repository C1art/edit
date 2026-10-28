use media::{AudioCodec, AudioSettings, RateControl, VideoCodec};
use playback::VideoExport;

/// What kind of file an export makes: its codecs and container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    H264,
    Hevc,
    ProRes422,
    ProRes422Hq,
    ProRes4444,
    Aac,
    Mp3,
    Wav,
    Flac,
}

impl Format {
    pub const ALL: [Self; 9] = [
        Self::H264,
        Self::Hevc,
        Self::ProRes422,
        Self::ProRes422Hq,
        Self::ProRes4444,
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
            Self::Aac => "AAC Audio",
            Self::Mp3 => "MP3",
            Self::Wav => "Waveform Audio",
            Self::Flac => "FLAC",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::H264 | Self::Hevc => "mp4",
            Self::ProRes422 | Self::ProRes422Hq | Self::ProRes4444 => "mov",
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
            Self::Aac | Self::Mp3 | Self::Wav | Self::Flac => None,
        }
    }

    pub fn audio_codec(self) -> AudioCodec {
        match self {
            Self::H264 | Self::Hevc | Self::Aac => AudioCodec::Aac,
            // Mastering formats carry uncompressed sound, as cameras and Premiere do.
            Self::ProRes422 | Self::ProRes422Hq | Self::ProRes4444 => AudioCodec::Pcm24,
            Self::Mp3 => AudioCodec::Mp3,
            Self::Wav => AudioCodec::Pcm16,
            Self::Flac => AudioCodec::Flac,
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
    /// Whether a video export carries sound; audio formats always do.
    pub with_audio: bool,
    pub audio_kbps: u32,
    pub sample_rate: u32,
}

pub struct Preset {
    pub name: &'static str,
    pub format: ExportFormat,
}

const MATCH_SOURCE: ExportFormat = ExportFormat {
    format: Format::H264,
    height: None,
    rate_control: RateControl::Quality(18),
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
    }
}

const fn format_preset(name: &'static str, format: Format) -> Preset {
    Preset {
        name,
        format: ExportFormat {
            format,
            ..MATCH_SOURCE
        },
    }
}

pub const PRESETS: [Preset; 12] = [
    Preset {
        name: "Match Source - High Bitrate",
        format: MATCH_SOURCE,
    },
    video_preset("YouTube 2160p 4K Ultra HD", 2160, 45),
    video_preset("YouTube 1080p Full HD", 1080, 16),
    video_preset("Vimeo 720p HD", 720, 10),
    Preset {
        name: "HEVC Match Source",
        format: ExportFormat {
            format: Format::Hevc,
            rate_control: RateControl::Quality(22),
            ..MATCH_SOURCE
        },
    },
    format_preset("Apple ProRes 422", Format::ProRes422),
    format_preset("Apple ProRes 422 HQ", Format::ProRes422Hq),
    format_preset("Apple ProRes 4444 with Alpha", Format::ProRes4444),
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
    pub fn preset_name(&self) -> Option<&'static str> {
        let used = self.used();
        PRESETS
            .iter()
            .find(|preset| preset.format.used() == used)
            .map(|preset| preset.name)
    }

    /// These settings with choices the format ignores put back to their defaults.
    fn used(self) -> Self {
        let unused = MATCH_SOURCE;
        let codec = self.format.video_codec();
        let rate_controlled = codec.is_some_and(VideoCodec::is_rate_controlled);
        Self {
            height: if codec.is_some() {
                self.height
            } else {
                unused.height
            },
            rate_control: if rate_controlled {
                self.rate_control
            } else {
                unused.rate_control
            },
            with_audio: codec.is_none() || self.with_audio,
            audio_kbps: if self.has_audio() && self.format.audio_codec().is_lossy() {
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
        self.with_audio || self.format.video_codec().is_none()
    }

    pub fn video(&self, sequence_size: (u32, u32)) -> Option<VideoExport> {
        Some(VideoExport {
            codec: self.format.video_codec()?,
            rate_control: self.rate_control,
            size: self.size(sequence_size),
        })
    }

    pub fn size(&self, sequence_size: (u32, u32)) -> (u32, u32) {
        self.height
            .map_or(sequence_size, |height| fit_height(sequence_size, height))
    }

    pub fn audio(&self) -> Option<AudioSettings> {
        self.has_audio().then(|| AudioSettings {
            codec: self.format.audio_codec(),
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
