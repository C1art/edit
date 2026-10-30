use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ClipId(pub u64);

/// Clips sharing a link move, trim and delete together, like the video and audio of
/// one source placed side by side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LinkId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TrackKind {
    Video,
    Audio,
}

/// Video tracks are numbered from the bottom layer up (V1 is under V2); audio tracks
/// from the top down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TrackRef {
    pub kind: TrackKind,
    pub index: usize,
}

impl TrackRef {
    pub const fn video(index: usize) -> Self {
        Self {
            kind: TrackKind::Video,
            index,
        }
    }

    pub const fn audio(index: usize) -> Self {
        Self {
            kind: TrackKind::Audio,
            index,
        }
    }
}

impl std::fmt::Display for TrackRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let letter = match self.kind {
            TrackKind::Video => 'V',
            TrackKind::Audio => 'A',
        };
        write!(f, "{letter}{}", self.index + 1)
    }
}
