use timeline::{Clip, ClipId, Edge, Sequence, Ticks};

use super::gesture::PendingEdit;
use super::TimelineView;

/// What the trim monitor shows while an edit is dragged.
pub struct TrimView<'a> {
    /// The sequence as the drag would leave it.
    pub sequence: &'a Sequence,
    /// The frames either side of the edit point, or a slipped clip's first and last.
    pub times: [Ticks; 2],
    /// How far the drag has moved the edit.
    pub delta: Ticks,
}

impl TimelineView {
    pub fn trim_view(&self) -> Option<TrimView<'_>> {
        let drag = self.drag.as_ref()?;
        let sequence = drag.preview.as_ref()?;
        let clip = |clips: &[ClipId]| -> Option<&Clip> {
            clips
                .first()
                .and_then(|&id| sequence.clip(id))
                .map(|(_, clip)| clip)
        };
        // A tick back lands in the frame before.
        let around = |at: Ticks| [(at - Ticks(1)).max(Ticks::ZERO), at];
        let (times, delta) = match drag.pending.as_ref()? {
            PendingEdit::Trim(trim) => {
                let clip = clip(&trim.clips)?;
                let at = match trim.edge {
                    Edge::Head => clip.start,
                    Edge::Tail => clip.end(),
                };
                (around(at), trim.delta)
            }
            PendingEdit::Roll(roll) => (around(roll.at + roll.delta), roll.delta),
            PendingEdit::Slip(slip) => {
                let clip = clip(&slip.clips)?;
                ([clip.start, clip.end() - Ticks(1)], slip.delta)
            }
            PendingEdit::Slide(slide) => {
                let clip = clip(&slide.clips)?;
                let [before, _] = around(clip.start);
                ([before, clip.end()], slide.delta)
            }
            _ => return None,
        };
        Some(TrimView {
            sequence,
            times,
            delta,
        })
    }
}
