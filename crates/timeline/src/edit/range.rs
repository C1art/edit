use std::ops::Range;

use crate::edit::ops::{self, Relinks};
use crate::{Clip, Edge, Edit, Marks, Sequence, Ticks, TimelineError};

/// Replaces the sequence's in and out marks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetMarks {
    pub marks: Marks,
}

impl Edit for SetMarks {
    fn name(&self) -> &'static str {
        "Set In/Out"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let Marks { mark_in, mark_out } = self.marks;
        if mark_in.is_some_and(|at| at < Ticks::ZERO) {
            return Err(TimelineError::BeforeSequenceStart);
        }
        if let (Some(mark_in), Some(mark_out)) = (mark_in, mark_out) {
            if mark_in >= mark_out {
                return Err(TimelineError::NonPositiveDuration);
            }
        }
        if sequence.marks == self.marks {
            return Err(TimelineError::NothingToEdit);
        }
        sequence.marks = self.marks;
        Ok(())
    }
}

/// Removes everything between the marks on unlocked tracks, leaving a gap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiftMarked;

impl Edit for LiftMarked {
    fn name(&self) -> &'static str {
        "Lift"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let range = marked(sequence)?;
        clear(sequence, range)
    }
}

/// Removes everything between the marks on unlocked tracks and closes the gap, then
/// clears the marks, since what they bounded is gone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractMarked;

impl Edit for ExtractMarked {
    fn name(&self) -> &'static str {
        "Extract"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let range = marked(sequence)?;
        extract(sequence, range)?;
        sequence.marks = Marks::default();
        Ok(())
    }
}

/// Removes the span between `at` and the nearest cut before it (`Edge::Head`) or after
/// it (`Edge::Tail`) among the clips under `at` on unlocked tracks, and closes it up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RippleTrimToPlayhead {
    pub at: Ticks,
    pub edge: Edge,
}

impl Edit for RippleTrimToPlayhead {
    fn name(&self) -> &'static str {
        match self.edge {
            Edge::Head => "Ripple Trim Previous Edit to Playhead",
            Edge::Tail => "Ripple Trim Next Edit to Playhead",
        }
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let span = self.span(sequence).ok_or(TimelineError::NothingToEdit)?;
        extract(sequence, span)
    }
}

impl RippleTrimToPlayhead {
    /// The span the trim removes.
    pub fn span(&self, sequence: &Sequence) -> Option<Range<Ticks>> {
        let under: Vec<&Clip> = ops::unlocked_tracks(sequence)
            .into_iter()
            .filter_map(|track| sequence.track(track)?.clip_at(self.at))
            .filter(|clip| clip.start < self.at)
            .collect();
        match self.edge {
            Edge::Head => under
                .iter()
                .map(|clip| clip.start)
                .max()
                .map(|cut| cut..self.at),
            Edge::Tail => under
                .iter()
                .map(|clip| clip.end())
                .min()
                .map(|cut| self.at..cut),
        }
    }
}

fn marked(sequence: &Sequence) -> Result<Range<Ticks>, TimelineError> {
    let range = sequence.marks.range(sequence.duration());
    if !sequence.marks.is_set() || range.is_empty() {
        return Err(TimelineError::NothingToEdit);
    }
    Ok(range)
}

fn clear(sequence: &mut Sequence, range: Range<Ticks>) -> Result<(), TimelineError> {
    let mut relinks = Relinks::default();
    for track in ops::unlocked_tracks(sequence) {
        ops::clear(sequence, track, range.clone(), &mut relinks)?;
    }
    Ok(())
}

fn extract(sequence: &mut Sequence, range: Range<Ticks>) -> Result<(), TimelineError> {
    clear(sequence, range.clone())?;
    let tracks = ops::unlocked_tracks(sequence);
    ops::shift(sequence, &tracks, range.end, range.start - range.end, &[]);
    Ok(())
}
