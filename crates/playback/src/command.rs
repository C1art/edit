use std::sync::Arc;

use timeline::{Sequence, Ticks};

use crate::{Levels, PlaybackError};

#[derive(Debug, Clone)]
pub(crate) enum Command {
    /// Takes effect from the next frame or chunk produced. The player follows it with a
    /// seek unless only levels changed.
    Sequence(Arc<Sequence>),
    Seek {
        position: Ticks,
        generation: u64,
    },
    Shutdown,
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
    pub(crate) seek: Option<(Ticks, u64)>,
}

/// Drains queued commands, keeping the latest sequence and the latest seek, so a burst
/// of scrubbing costs one seek. Returns `None` on shutdown or disconnection.
pub(crate) fn coalesce(
    first: Command,
    commands: &crossbeam::channel::Receiver<Command>,
) -> Option<Coalesced> {
    let mut sequence = None;
    let mut seek = None;
    let mut next = Some(first);
    while let Some(command) = next {
        match command {
            Command::Sequence(new) => sequence = Some(new),
            Command::Seek {
                position,
                generation,
            } => seek = Some((position, generation)),
            Command::Shutdown => return None,
        }
        next = match commands.try_recv() {
            Ok(command) => Some(command),
            Err(crossbeam::channel::TryRecvError::Empty) => None,
            Err(crossbeam::channel::TryRecvError::Disconnected) => return None,
        };
    }
    Some(Coalesced { sequence, seek })
}
