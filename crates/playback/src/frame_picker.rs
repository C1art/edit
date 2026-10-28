use timeline::Ticks;

use crate::ComposedFrame;

/// A decoded frame labelled with the seek it belongs to, so frames decoded before a
/// seek can be told apart from frames decoded after it.
#[derive(Debug)]
pub(crate) struct TaggedFrame {
    pub(crate) generation: u64,
    pub(crate) frame: ComposedFrame,
}

/// Chooses the frame to show for the clock position: frames whose time has passed are
/// skipped (dropped), and the current frame stays up until the next one is due (repeated).
#[derive(Debug, Default)]
pub(crate) struct FramePicker {
    generation: u64,
    current: Option<ComposedFrame>,
    next: Option<ComposedFrame>,
    /// The current frame is from before a seek; replace it with the first new frame
    /// regardless of its timestamp.
    stale: bool,
}

impl FramePicker {
    pub(crate) fn current(&self) -> Option<&ComposedFrame> {
        self.current.as_ref()
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.next.is_some()
    }

    /// Keeps showing the current frame until the first frame of `generation` arrives,
    /// which avoids flashing black while scrubbing.
    pub(crate) fn reset(&mut self, generation: u64) {
        self.generation = generation;
        self.next = None;
        self.stale = true;
    }

    /// Returns `true` when the frame to show changed.
    pub(crate) fn advance(
        &mut self,
        position: Ticks,
        mut receive: impl FnMut() -> Option<TaggedFrame>,
    ) -> bool {
        let mut changed = false;
        loop {
            if self.next.is_none() {
                match receive() {
                    Some(tagged) if tagged.generation == self.generation => {
                        self.next = Some(tagged.frame);
                    }
                    Some(_) => continue,
                    None => return changed,
                }
            }
            let due = self
                .next
                .as_ref()
                .is_some_and(|next| self.stale || self.current.is_none() || next.pts <= position);
            if !due {
                return changed;
            }
            self.current = self.next.take();
            self.stale = false;
            changed = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use super::*;

    fn tagged(generation: u64, pts: i64) -> TaggedFrame {
        TaggedFrame {
            generation,
            frame: ComposedFrame {
                pts: Ticks(pts),
                layers: Vec::new(),
            },
        }
    }

    fn advance(picker: &mut FramePicker, position: i64, queue: &mut VecDeque<TaggedFrame>) -> bool {
        picker.advance(Ticks(position), || queue.pop_front())
    }

    fn current_pts(picker: &FramePicker) -> Option<i64> {
        picker.current().map(|frame| frame.pts.0)
    }

    #[test]
    fn drops_late_frames_and_holds_early_ones() {
        let mut picker = FramePicker::default();
        let mut queue: VecDeque<_> = [0, 40, 80, 120].map(|pts| tagged(0, pts)).into();
        assert!(advance(&mut picker, 90, &mut queue));
        assert_eq!(current_pts(&picker), Some(80));
        assert!(picker.has_pending());
        assert!(!advance(&mut picker, 100, &mut queue));
        assert_eq!(current_pts(&picker), Some(80));
    }

    #[test]
    fn shows_the_first_frame_even_if_it_is_ahead_of_the_clock() {
        let mut picker = FramePicker::default();
        let mut queue: VecDeque<_> = [tagged(0, 3)].into();
        assert!(advance(&mut picker, 0, &mut queue));
        assert_eq!(current_pts(&picker), Some(3));
    }

    #[test]
    fn after_a_seek_ignores_old_frames_and_replaces_the_stale_one() {
        let mut picker = FramePicker::default();
        let mut queue: VecDeque<_> = [tagged(0, 0)].into();
        advance(&mut picker, 0, &mut queue);
        picker.reset(1);
        let mut queue: VecDeque<_> = [tagged(0, 40), tagged(1, 1000), tagged(1, 1040)].into();
        assert!(advance(&mut picker, 0, &mut queue));
        assert_eq!(current_pts(&picker), Some(1000));
    }
}
