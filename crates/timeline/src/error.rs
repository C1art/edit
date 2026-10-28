use std::fmt;

use crate::{Ticks, TrackIndex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimelineError {
    TrackNotFound(TrackIndex),
    ClipOverlap { track: TrackIndex, at: Ticks },
    NonPositiveDuration,
}

impl fmt::Display for TimelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TrackNotFound(track) => write!(f, "track {} does not exist", track.0),
            Self::ClipOverlap { track, at } => write!(
                f,
                "clip overlaps another clip on track {} at tick {}",
                track.0, at.0
            ),
            Self::NonPositiveDuration => f.write_str("clip duration must be positive"),
        }
    }
}

impl std::error::Error for TimelineError {}
