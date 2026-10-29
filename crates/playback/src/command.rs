use std::sync::Arc;

use timeline::{Sequence, Ticks};

use crate::previews::PreviewList;
use crate::{Levels, PlaybackError, Resolution};

#[derive(Debug, Clone)]
pub(crate) enum Command {
    /// Takes effect from the next frame or chunk produced. The player follows it with a
    /// seek unless only levels changed.
    Sequence(Arc<Sequence>),
    Seek(Seek),
    /// Only the video worker acts on it; the player follows it with a seek.
    Resolution(Resolution),
    /// Only the video worker acts on it.
    Previews(PreviewList),
    Shutdown,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Seek {
    pub(crate) position: Ticks,
    pub(crate) generation: u64,
    /// Sounds a short stretch from `position` while paused, rather than readying the
    /// audio to play on from there.
    pub(crate) scrub: bool,
}

#[derive(Debug)]
pub(crate) enum Event {
    /// The worker that owns the clock restarted output for this seek; for audio, the
    /// queue was emptied when the device had played `frames_played`.
    Seeked { generation: u64, frames_played: u64 },
    /// Video for this seek has been produced up to the end of the sequence.
    Ended { generation: u64 },
    /// A problem with one piece of media; playback carries on without it.
    Warning(PlaybackError),
    /// Peak levels of one mixed chunk, for meters.
    Levels(Levels),
}

/// The net effect of a burst of commands.
pub(crate) struct Coalesced {
    pub(crate) sequence: Option<Arc<Sequence>>,
    pub(crate) seek: Option<Seek>,
    pub(crate) resolution: Option<Resolution>,
    pub(crate) previews: Option<PreviewList>,
}

/// Drains queued commands, keeping the latest sequence and the latest seek, so a burst
/// of scrubbing costs one seek. Returns `None` on shutdown or disconnection.
pub(crate) fn coalesce(
    first: Command,
    commands: &crossbeam::channel::Receiver<Command>,
) -> Option<Coalesced> {
    let mut sequence = None;
    let mut seek = None;
    let mut resolution = None;
    let mut previews = None;
    let mut next = Some(first);
    while let Some(command) = next {
        match command {
            Command::Sequence(new) => sequence = Some(new),
            Command::Seek(new) => seek = Some(new),
            Command::Resolution(new) => resolution = Some(new),
            Command::Previews(new) => previews = Some(new),
            Command::Shutdown => return None,
        }
        next = match commands.try_recv() {
            Ok(command) => Some(command),
            Err(crossbeam::channel::TryRecvError::Empty) => None,
            Err(crossbeam::channel::TryRecvError::Disconnected) => return None,
        };
    }
    Some(Coalesced {
        sequence,
        seek,
        resolution,
        previews,
    })
}
