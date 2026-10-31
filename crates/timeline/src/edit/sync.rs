use crate::edit::ops;
use crate::{ClipId, Edit, Sequence, Ticks, TimelineError};

/// How an out-of-sync clip is brought back in line with its link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMethod {
    /// Moves the clip along its track, keeping what it plays.
    Move,
    /// Keeps the clip in place and plays other media in it.
    Slip,
}

/// Premiere's Move into Sync and Slip into Sync on a clip whose link has drifted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntoSync {
    pub clip: ClipId,
    pub method: SyncMethod,
}

impl Edit for IntoSync {
    fn name(&self) -> &'static str {
        match self.method {
            SyncMethod::Move => "Move into Sync",
            SyncMethod::Slip => "Slip into Sync",
        }
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let (track, clip) = sequence
            .clip(self.clip)
            .ok_or(TimelineError::ClipNotFound(self.clip))?;
        let (other, offset) = sequence
            .out_of_sync_with(clip)
            .ok_or(TimelineError::NothingToEdit)?;
        // A reversed clip plays its first source frame last, so the frame to take from
        // the partner is the one it plays where this clip plays that frame.
        let source_in = other.source_time(clip.position_of(clip.source_in));
        let clip = ops::clip_mut(sequence, self.clip)?;
        match self.method {
            SyncMethod::Move => clip.start = clip.start - offset,
            SyncMethod::Slip => {
                let old_in = clip.source_in;
                clip.source_in = source_in;
                if let Some(remap) = &mut clip.remap {
                    remap.shift(Ticks::ZERO, source_in - old_in);
                }
                clip.carry_pins_through_slip(old_in);
            }
        }
        // Moving may carry it past a neighbour on its track.
        ops::track_mut(sequence, track)?
            .clips
            .sort_by_key(|clip| clip.start);
        Ok(())
    }
}
