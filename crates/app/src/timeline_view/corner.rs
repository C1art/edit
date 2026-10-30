use egui::{Rect, UiBuilder};
use timeline::{Sequence, Ticks, TimeDisplay};

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
        let format = (
            sequence.timebase,
            sequence.format.frame_rate,
            sequence.time_display,
        );
        let field = timecode_field::playhead(&mut corner, id, playhead, format, 15.0);
        output.seek = field.seek.or(output.seek);
        field.response.context_menu(|ui| {
            let rate = sequence.format.frame_rate;
            for choice in TimeDisplay::choices(rate) {
                let chosen = sequence.time_display == choice;
                if ui.radio(chosen, choice.name(rate)).clicked() {
                    output.time_display = Some(choice);
                    ui.close();
                }
            }
        });
    }
}
