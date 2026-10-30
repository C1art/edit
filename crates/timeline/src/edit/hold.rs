use std::sync::Arc;

use crate::edit::ops::{self, Relinks};
use crate::{Clip, ClipId, Edit, Sequence, Ticks, TimelineError, TrackKind};

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

/// Cuts the video clips among `clips` at `at` and puts `length` of the frame showing
/// there between the two pieces, pushing everything later on the rippled tracks along.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InsertFrameHold {
    pub clips: Vec<ClipId>,
    pub at: Ticks,
    pub length: Ticks,
}

impl Edit for InsertFrameHold {
    fn name(&self) -> &'static str {
        "Insert Frame Hold Segment"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        if self.length <= Ticks::ZERO {
            return Err(TimelineError::NothingToEdit);
        }
        let held: Vec<_> = self
            .clips
            .iter()
            .filter_map(|&id| sequence.clip(id))
            .filter(|(track, clip)| {
                track.kind == TrackKind::Video
                    && clip.covers(self.at)
                    && sequence.track(*track).is_some_and(|track| !track.locked)
            })
            .map(|(track, clip)| (track, Clip::clone(clip)))
            .collect();
        if held.is_empty() {
            return Err(TimelineError::NothingToEdit);
        }
        let edited: Vec<_> = held.iter().map(|&(track, _)| track).collect();
        let rippled = ops::rippled_tracks(sequence, &edited);
        ops::split(sequence, &rippled, self.at, &mut Relinks::default());
        ops::shift(sequence, &rippled, self.at, self.length, &[]);
        ops::ripple_markers(sequence, self.at, self.length);
        for (track, clip) in held {
            let hold = Clip {
                id: ClipId(sequence.ids.next()),
                start: self.at,
                duration: self.length,
                source_in: clip.source_time(self.at),
                hold: true,
                link: None,
                transition_in: None,
                transition_out: None,
                ..clip
            };
            ops::insert(sequence, track, hold)?;
        }
        Ok(())
    }
}
