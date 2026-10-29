use std::path::Path;

use timeline::{Ticks, Timebase};

use crate::sequence::open_input;
use crate::MediaError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamKind {
    Video,
    Audio,
    Subtitle,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamInfo {
    pub index: usize,
    pub kind: StreamKind,
    pub timebase: Timebase,
    /// In `timebase`; `None` when the container doesn't declare it.
    pub duration: Option<Ticks>,
    /// FFmpeg's short name for the codec, as "h264".
    pub codec: String,
    /// Bits per second.
    pub bit_rate: Option<u64>,
    /// Audio streams only.
    pub sample_rate: Option<u32>,
    /// Audio streams only.
    pub channels: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaInfo {
    pub streams: Vec<StreamInfo>,
    /// The container's descriptive name.
    pub container: String,
    /// Bits per second over the whole file.
    pub bit_rate: Option<u64>,
}

/// Opens the file and reads its stream headers. Blocks on file I/O, so call it from a worker.
pub fn probe(path: &Path) -> Result<MediaInfo, MediaError> {
    let (input, _) = open_input(path)?;
    Ok(MediaInfo {
        streams: input.streams(),
        container: input.container(),
        bit_rate: input.bit_rate(),
    })
}
