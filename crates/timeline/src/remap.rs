use serde::{Deserialize, Serialize};

use crate::{Speed, Ticks};

/// Pins a source time to an offset from the clip's start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Anchor {
    pub offset: Ticks,
    pub source: Ticks,
}

/// Time remapping: a video clip whose speed changes along its length. It plays at a
/// constant speed between anchors and, beyond the first and last, at the speed of the
/// nearest segment, so trimming never needs new anchors. Anchors inside the clip are
/// its speed keyframes, and they stay on their source frame when speeds change.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TimeRemap {
    /// At least two, with offsets strictly and sources weakly increasing.
    anchors: Vec<Anchor>,
}

impl TimeRemap {
    /// Plays `duration` ticks from `source_in` at `speed`, as a clip without remapping.
    pub fn constant(source_in: Ticks, duration: Ticks, speed: Speed) -> Self {
        let duration = duration.max(Ticks(1));
        Self {
            anchors: vec![
                Anchor {
                    offset: Ticks::ZERO,
                    source: source_in,
                },
                Anchor {
                    offset: duration,
                    source: source_in + speed.source_span(duration),
                },
            ],
        }
    }

    pub fn anchors(&self) -> &[Anchor] {
        &self.anchors
    }

    /// Each segment's speed, every one between 1% and 10,000%.
    pub fn is_valid(&self) -> bool {
        self.anchors.len() >= 2
            && self.anchors.windows(2).all(|pair| {
                Speed::stretch(
                    pair[1].source - pair[0].source,
                    pair[1].offset - pair[0].offset,
                )
                .is_some_and(Speed::is_in_range)
            })
    }

    /// The speed of the segment from anchor `index` to the next.
    pub fn speed(&self, index: usize) -> Option<Speed> {
        let (from, to) = (self.anchors.get(index)?, self.anchors.get(index + 1)?);
        Speed::stretch(to.source - from.source, to.offset - from.offset)
    }

    /// The source time playing `offset` ticks into the clip, rounded down.
    pub fn source_at(&self, offset: Ticks) -> Ticks {
        let (from, to) = self.segment(self.anchors.partition_point(|a| a.offset <= offset));
        from.source
            + scale(
                offset - from.offset,
                to.source - from.source,
                to.offset - from.offset,
            )
    }

    /// The offset where `source` plays, rounded down.
    pub fn offset_of(&self, source: Ticks) -> Ticks {
        let (from, to) = self.segment(self.anchors.partition_point(|a| a.source <= source));
        if to.source == from.source {
            return from.offset;
        }
        from.offset
            + scale(
                source - from.source,
                to.offset - from.offset,
                to.source - from.source,
            )
    }

    /// The anchors either side of the one that would sit at `after`, clamped to a real
    /// segment so times beyond the ends extrapolate.
    fn segment(&self, after: usize) -> (Anchor, Anchor) {
        let index = after.clamp(1, self.anchors.len().max(2) - 1);
        (self.anchors[index - 1], self.anchors[index])
    }

    pub(crate) fn shift(&mut self, offset: Ticks, source: Ticks) {
        for anchor in &mut self.anchors {
            anchor.offset = anchor.offset + offset;
            anchor.source = anchor.source + source;
        }
    }

    /// Keyframes the source playing at `offset`; `false` if one is already there.
    pub(crate) fn insert(&mut self, offset: Ticks) -> bool {
        match self.anchors.binary_search_by_key(&offset, |a| a.offset) {
            Ok(_) => false,
            Err(index) => {
                let source = self.source_at(offset);
                self.anchors.insert(index, Anchor { offset, source });
                true
            }
        }
    }

    /// Joins the segments either side of the anchor at `offset`.
    pub(crate) fn remove(&mut self, offset: Ticks) -> bool {
        let found = self.anchors.binary_search_by_key(&offset, |a| a.offset);
        match found {
            Ok(index) if self.anchors.len() > 2 => {
                self.anchors.remove(index);
                true
            }
            _ => false,
        }
    }

    /// Plays the segment from anchor `index` at `speed` or slower, moving the anchors
    /// after it.
    pub(crate) fn set_speed(&mut self, index: usize, speed: Speed) -> bool {
        let Some(&[from, to]) = self.anchors.get(index..index + 2) else {
            return false;
        };
        let span = Ticks(-speed.sequence_span(from.source - to.source).0).max(Ticks(1));
        let change = span - (to.offset - from.offset);
        for anchor in &mut self.anchors[index + 1..] {
            anchor.offset = anchor.offset + change;
        }
        change != Ticks::ZERO
    }

    /// Spreads the anchors over `to` ticks instead of `from`, as a rate stretch does.
    pub(crate) fn stretch(&mut self, from: Ticks, to: Ticks) {
        for anchor in &mut self.anchors {
            anchor.offset = scale(anchor.offset, to, from);
        }
        self.anchors.dedup_by_key(|a| a.offset);
    }
}

/// `ticks * num / den`, rounded toward negative infinity.
fn scale(ticks: Ticks, num: Ticks, den: Ticks) -> Ticks {
    let product = i128::from(ticks.0) * i128::from(num.0);
    let quotient = product.div_euclid(i128::from(den.0.max(1)));
    Ticks(i64::try_from(quotient).unwrap_or(if quotient < 0 { i64::MIN } else { i64::MAX }))
}
