use egui::Response;
use timeline::{Sequence, Ticks};

use super::scale::{self, TimeScale};
use super::{markers, TimelineView, SNAP_POINTS};

impl TimelineView {
    /// Where a press or drag on the ruler puts the playhead: on a clicked marker, else the
    /// frame under the pointer, drawn to a nearby cut or marker while snapping is on
    /// (Shift flips it), as Premiere's playhead does.
    pub(super) fn scrub(
        &self,
        response: &Response,
        sequence: &Sequence,
        scale: &TimeScale,
    ) -> Option<Ticks> {
        let pointer = response.interact_pointer_pos()?;
        let marker = markers::hit(scale, sequence, pointer.x)
            .filter(|_| !response.dragged())
            .and_then(|id| sequence.markers.iter().find(|marker| marker.id == id));
        if let Some(marker) = marker {
            return Some(marker.at);
        }
        let time = scale::nearest_frame(
            scale.time(pointer.x).max(Ticks::ZERO),
            sequence.format.frame_rate,
            sequence.timebase,
        );
        let shift = response.ctx.input(|input| input.modifiers.shift);
        if self.snapping == shift {
            return Some(time);
        }
        let targets = scale::snap_targets(sequence, &[], Ticks::ZERO);
        Some(time + scale::snap(Ticks::ZERO, &[time], &targets, scale.span(SNAP_POINTS)))
    }
}
