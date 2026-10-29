use egui::{Rect, UiBuilder};
use timeline::{Sequence, Ticks};

use super::{TimelineOutput, TimelineView};
use crate::timecode_field;

impl TimelineView {
    /// The playhead's timecode over the track headers, where Premiere has it, for
    /// typing where to go.
    pub(super) fn corner_timecode(
        &self,
        ui: &mut egui::Ui,
        corner: Rect,
        sequence: &Sequence,
        playhead: Ticks,
        output: &mut TimelineOutput,
    ) {
        let builder = UiBuilder::new()
            .max_rect(corner.shrink2(egui::vec2(8.0, 0.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center));
        let mut corner = ui.new_child(builder);
        let id = corner.scope_id().with("playhead timecode");
        let format = (sequence.timebase, sequence.format.frame_rate);
        if let Some(time) = timecode_field::playhead(&mut corner, id, playhead, format, 15.0) {
            output.seek = Some(time);
        }
    }
}
