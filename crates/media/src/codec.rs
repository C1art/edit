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
}

impl VideoCodec {
    /// Whether [`RateControl`] applies.
    pub fn is_rate_controlled(self) -> bool {
        matches!(self, Self::H264 | Self::Hevc)
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
