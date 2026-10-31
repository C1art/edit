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
        ops::ripple_markers_and_captions(sequence, self.at, self.length);
        for (track, clip) in held {
            // The remap is measured from the clip's start, so it's moved to the hold's,
            // which a released hold then plays on from.
            let remap = clip.remap.clone().map(|mut remap| {
                remap.shift(clip.start - self.at, Ticks::ZERO);
                remap
            });
            let hold = Clip {
                remap,
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

/// Which of its own frames a whole clip holds on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldFrame {
    In,
    Out,
    /// The frame showing at this sequence position, for clips over it.
    At(Ticks),
}

/// Makes the video clips among `clips` show one frame for their whole length, as
/// Premiere's Frame Hold Options do, or with `None` play on again from the frame held.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetFrameHold {
    pub clips: Vec<ClipId>,
    pub frame: Option<HoldFrame>,
}

impl Edit for SetFrameHold {
    fn name(&self) -> &'static str {
        match self.frame {
            Some(_) => "Frame Hold Options",
            None => "Remove Frame Hold",
        }
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let changes: Vec<_> = self
            .clips
            .iter()
            .filter_map(|&id| sequence.clip(id))
            .filter(|(track, _)| track.kind == TrackKind::Video)
            .filter_map(|(_, clip)| {
                let held = match self.frame {
                    None => {
                        return clip
                            .hold
                            .then(|| (clip.id, false, released_in(sequence, clip)))
                    }
                    Some(HoldFrame::In) => clip.source_time(clip.start),
                    Some(HoldFrame::Out) => clip.source_time(clip.end() - Ticks(1)),
                    Some(HoldFrame::At(at)) => clip.covers(at).then(|| clip.source_time(at))?,
                };
                let unchanged = clip.hold && clip.source_in == held;
                (!unchanged).then_some((clip.id, true, held))
            })
            .collect();
        if changes.is_empty() {
            return Err(TimelineError::NothingToEdit);
        }
        for (id, hold, source_in) in changes {
            let clip = ops::clip_mut(sequence, id)?;
            clip.hold = hold;
            clip.source_in = source_in;
        }
        Ok(())
    }
}

/// Where a released clip starts playing from: the frame it held, unless that leaves too
/// little media after it to fill the clip, when it starts as late as the media allows.
fn released_in(sequence: &Sequence, clip: &Clip) -> Ticks {
    let Some(item) = sequence
        .media_item(clip.source)
        .filter(|_| clip.remap.is_none())
    else {
        return clip.source_in;
    };
    let span = clip.rate().source_span(clip.duration);
    clip.source_in.min(item.duration - span).max(Ticks::ZERO)
}
