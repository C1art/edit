use std::fmt;

use crate::{ClipId, MarkerId, SourceId, Ticks, TrackRef};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimelineError {
    TrackNotFound(TrackRef),
    TrackLocked(TrackRef),
    ClipNotFound(ClipId),
    MarkerNotFound(MarkerId),
    MediaNotFound(SourceId),
    MediaInUse(SourceId),
    ClipOverlap {
        track: TrackRef,
        at: Ticks,
    },
    NonPositiveDuration,
    MarkersOutOfOrder,
    SpeedOutOfRange(ClipId),
    BeforeSequenceStart,
    /// The clip would need source material before the start or after the end of its media.
    OutsideMedia(ClipId),
    /// The edit has no clip or track to act on.
    NothingToEdit,
}

impl fmt::Display for TimelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TrackNotFound(track) => write!(f, "track {track} does not exist"),
            Self::TrackLocked(track) => write!(f, "track {track} is locked"),
            Self::ClipNotFound(clip) => write!(f, "clip {} does not exist", clip.0),
            Self::MarkerNotFound(marker) => write!(f, "marker {} does not exist", marker.0),
            Self::MediaNotFound(source) => write!(f, "media {} is not in the project", source.0),
            Self::MediaInUse(source) => write!(f, "media {} is used in the sequence", source.0),
            Self::ClipOverlap { track, at } => {
                write!(
                    f,
                    "clip overlaps another clip on track {track} at tick {}",
                    at.0
                )
            }
            Self::MarkersOutOfOrder => f.write_str("markers are out of time order"),
            Self::SpeedOutOfRange(_) => f.write_str("clip speed must be between 1% and 10,000%"),
            Self::NonPositiveDuration => f.write_str("clip duration must be positive"),
            Self::BeforeSequenceStart => f.write_str("nothing can start before the sequence"),
            Self::OutsideMedia(_) => f.write_str("not enough source media for this edit"),
            Self::NothingToEdit => f.write_str("nothing to edit"),
        }
    }
}

impl std::error::Error for TimelineError {}
