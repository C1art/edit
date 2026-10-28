/// How an H.264 or HEVC encoder spends bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateControl {
    /// Constant rate factor: lower is better and larger; 18 looks lossless on most
    /// H.264 material.
    Quality(u8),
    /// An average, in bits per second, for platforms that cap the rate.
    Bitrate(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VideoCodec {
    #[default]
    H264,
    Hevc,
    /// Apple ProRes 4:2:2 10-bit, which sets its own data rate.
    ProRes422,
    ProRes422Hq,
    /// Apple ProRes 4:4:4:4 with a 16-bit alpha channel, for graphics laid over
    /// other footage.
    ProRes4444,
    /// Avid DNxHR, which works at any frame size from 256 × 120 up.
    Dnxhr(DnxhrQuality),
}

/// DNxHR's profiles, from smallest to best.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DnxhrQuality {
    Lb,
    Sq,
    Hq,
    /// 10-bit.
    Hqx,
    /// 10-bit 4:4:4.
    Dnx444,
}

impl DnxhrQuality {
    pub const ALL: [Self; 5] = [Self::Lb, Self::Sq, Self::Hq, Self::Hqx, Self::Dnx444];

    pub fn name(self) -> &'static str {
        match self {
            Self::Lb => "LB",
            Self::Sq => "SQ",
            Self::Hq => "HQ",
            Self::Hqx => "HQX 10-bit",
            Self::Dnx444 => "444 10-bit",
        }
    }
}

impl VideoCodec {
    /// Whether [`RateControl`] applies.
    pub fn is_rate_controlled(self) -> bool {
        matches!(self, Self::H264 | Self::Hevc)
    }

    /// Whether the picture's transparency is kept.
    pub fn keeps_alpha(self) -> bool {
        self == Self::ProRes4444
    }

    pub(crate) fn is_prores(self) -> bool {
        matches!(self, Self::ProRes422 | Self::ProRes422Hq | Self::ProRes4444)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AudioCodec {
    #[default]
    Aac,
    Mp3,
    Flac,
    Pcm16,
    Pcm24,
}

impl AudioCodec {
    /// Whether the bitrate setting applies; lossless codecs take what they need.
    pub fn is_lossy(self) -> bool {
        matches!(self, Self::Aac | Self::Mp3)
    }
}
