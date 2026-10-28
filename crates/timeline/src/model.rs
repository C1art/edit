use serde::{Deserialize, Serialize};

use crate::{Ticks, Timebase};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ClipId(pub u64);

/// Identifies a media item in the project's media pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TrackIndex(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackKind {
    Video,
    Audio,
}

/// All times are in the owning sequence's timebase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clip {
    pub id: ClipId,
    pub source: SourceId,
    pub start: Ticks,
    pub duration: Ticks,
    pub source_in: Ticks,
}

impl Clip {
    pub fn end(&self) -> Ticks {
        self.start + self.duration
    }

    pub fn overlaps(&self, other: &Clip) -> bool {
        self.start < other.end() && other.start < self.end()
    }
}

/// Clips are kept sorted by `start` and never overlap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    pub kind: TrackKind,
    pub clips: Vec<Clip>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sequence {
    pub timebase: Timebase,
    pub tracks: Vec<Track>,
}

impl Sequence {
    pub fn new(timebase: Timebase) -> Self {
        Self {
            timebase,
            tracks: Vec::new(),
        }
    }
}
