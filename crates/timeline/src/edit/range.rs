use std::ops::Range;

use crate::edit::ops::{self, Relinks};
use crate::{Edit, Marks, Sequence, Ticks, TimelineError};

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
        clear_marked(sequence)?;
        Ok(())
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
        let range = clear_marked(sequence)?;
        let tracks = ops::unlocked_tracks(sequence);
        ops::shift(sequence, &tracks, range.end, range.start - range.end, &[]);
        sequence.marks = Marks::default();
        Ok(())
    }
}

fn clear_marked(sequence: &mut Sequence) -> Result<Range<Ticks>, TimelineError> {
    if !sequence.marks.is_set() {
        return Err(TimelineError::NothingToEdit);
    }
    let range = sequence.marks.range(sequence.duration());
    if range.is_empty() {
        return Err(TimelineError::NothingToEdit);
    }
    let mut relinks = Relinks::default();
    for track in ops::unlocked_tracks(sequence) {
        ops::clear(sequence, track, range.clone(), &mut relinks)?;
    }
    Ok(range)
}
