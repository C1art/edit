use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::Ticks;

/// In and out points bounding the part of a sequence or source that edits and exports
/// work on. Either may be missing, standing for the start or the end.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marks {
    #[serde(default)]
    pub mark_in: Option<Ticks>,
    #[serde(default)]
    pub mark_out: Option<Ticks>,
}

impl Marks {
    /// Marks in at `at`, dropping an out mark that would no longer follow it.
    pub fn with_in(self, at: Ticks) -> Self {
        Self {
            mark_in: Some(at),
            mark_out: self.mark_out.filter(|&out| out > at),
        }
    }

    /// Marks out at `at`, dropping an in mark that would no longer precede it.
    pub fn with_out(self, at: Ticks) -> Self {
        Self {
            mark_in: self.mark_in.filter(|&mark_in| mark_in < at),
            mark_out: Some(at),
        }
    }

    pub fn is_set(self) -> bool {
        self.mark_in.is_some() || self.mark_out.is_some()
    }

    /// The marked range, running from the start or to `end` where a mark is missing.
    pub fn range(self, end: Ticks) -> Range<Ticks> {
        self.mark_in.unwrap_or(Ticks::ZERO)..self.mark_out.unwrap_or(end)
    }
}
