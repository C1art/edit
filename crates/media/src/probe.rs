use std::path::Path;

use timeline::{Ticks, Timebase};

use crate::ffi::InputContext;
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaInfo {
    pub streams: Vec<StreamInfo>,
}

/// Opens the file and reads its stream headers. Blocks on file I/O, so call it from a worker.
pub fn probe(path: &Path) -> Result<MediaInfo, MediaError> {
    let input = InputContext::open(path)?;
    Ok(MediaInfo {
        streams: input.streams(),
    })
}
