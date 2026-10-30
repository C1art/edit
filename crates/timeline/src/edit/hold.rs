use std::sync::Arc;

use crate::edit::ops::{self, Relinks};
use crate::{ClipId, Edit, Sequence, Ticks, TimelineError, TrackKind};

/// Freezes the video clips among `clips` on the frame showing at `at`, from there to
/// their ends. A clip starting before `at` is cut there, its held tail unlinked from
/// the sound, which plays on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddFrameHold {
    pub clips: Vec<ClipId>,
    pub at: Ticks,
}

impl Edit for AddFrameHold {
    fn name(&self) -> &'static str {
        "Add Frame Hold"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let holds: Vec<_> = self
            .clips
            .iter()
            .filter_map(|&id| sequence.clip(id))
            .filter(|(track, clip)| {
                track.kind == TrackKind::Video && clip.covers(self.at) && !clip.hold
            })
            .map(|(track, clip)| (track, clip.source_time(self.at)))
            .collect();
        if holds.is_empty() {
            return Err(TimelineError::NothingToEdit);
        }
        let mut relinks = Relinks::default();
        for (track, held) in holds {
            ops::split(sequence, &[track], self.at, &mut relinks);
            let tail = ops::track_mut(sequence, track)?
                .clips
                .iter_mut()
                .find(|clip| clip.start == self.at)
                .map(Arc::make_mut)
                .ok_or(TimelineError::NothingToEdit)?;
            tail.source_in = held;
            tail.hold = true;
        }
        Ok(())
    }
}
