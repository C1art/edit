//! How long a still or generated media places for when nothing is marked, kept with
//! each item as it comes in, as Premiere's Still Image Default Duration is.

use std::ops::RangeInclusive;

use timeline::{Sequence, Ticks};

use crate::transition_defaults::whole_frames;

/// Hundredths of a second.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StillLength(pub u32);

impl StillLength {
    pub const STANDARD: Self = Self(500);
    pub const LENGTHS: RangeInclusive<u32> = 10..=60_000;

    /// A whole number of `sequence`'s frames, or `None` at the standard length, which
    /// items without one of their own place for.
    pub fn placed(self, sequence: &Sequence) -> Option<Ticks> {
        (self != Self::STANDARD).then(|| {
            let rate = sequence.format.frame_rate;
            let frames = whole_frames(self.0, rate.num(), rate.den());
            rate.frame_start(frames, sequence.timebase)
        })
    }
}
