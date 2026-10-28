use std::fmt;

use crate::{BinId, ClipId, MarkerId, SourceId, Ticks, TrackRef};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimelineError {
    TrackNotFound(TrackRef),
    TrackLocked(TrackRef),
    ClipNotFound(ClipId),
    MarkerNotFound(MarkerId),
    MediaNotFound(SourceId),
    MediaInUse(SourceId),
    BinNotFound(BinId),
    /// The bin would go inside itself.
    BinCycle(BinId),
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
    /// The marks ask for more of the source than it has.
    NotEnoughSource,
    /// The edit has no clip or track to act on.
    NothingToEdit,
    /// The nested sequence would show itself.
    NestCycle(SourceId),
    /// A caption ends before it starts or overlaps the one before.
    CaptionOutOfOrder(Ticks),
    /// A subtitle file's timing can't be read at this line, counting from one.
    InvalidSubtitles {
        line: usize,
    },
    /// An interchange file isn't well-formed XML at this line, counting from one.
    InvalidXml {
        line: usize,
    },
    /// An interchange file is XML but holds no sequence this editor can read.
    UnreadableSequence(String),
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
            Self::BinNotFound(bin) => write!(f, "bin {} does not exist", bin.0),
            Self::BinCycle(_) => f.write_str("a bin can't go inside itself"),
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
            Self::NotEnoughSource => {
                f.write_str("the source has too little media for the sequence marks")
            }
            Self::NothingToEdit => f.write_str("nothing to edit"),
            Self::NestCycle(_) => f.write_str("a sequence can't contain itself"),
            Self::CaptionOutOfOrder(_) => {
                f.write_str("captions must not overlap and must end after they start")
            }
            Self::InvalidSubtitles { line } => {
                write!(f, "subtitle timing on line {line} is not HH:MM:SS,mmm")
            }
            Self::InvalidXml { line } => write!(f, "the XML is malformed at line {line}"),
            Self::UnreadableSequence(reason) => write!(f, "cannot read the sequence: {reason}"),
        }
    }
}

impl std::error::Error for TimelineError {}
