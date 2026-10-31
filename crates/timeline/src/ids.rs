use std::sync::atomic::{AtomicU64, Ordering};

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

/// Which track this is while the program runs, wherever adding and deleting tracks
/// moves it, so a view can keep its own state for the track. Each new track gets its
/// own, a copy keeps it, and a loaded one gets a new one; it isn't saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TrackId(u64);

impl Default for TrackId {
    fn default() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
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
