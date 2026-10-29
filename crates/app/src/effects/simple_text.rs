use timeline::{SimpleText, TextAlign, VideoEffect};

use super::look::Rows;
use super::Change;

const ALIGNMENTS: [TextAlign; 3] = [TextAlign::Left, TextAlign::Center, TextAlign::Right];

impl Rows {
    /// The controls of Simple Text; other effects have none here.
    pub(super) fn simple_text(&mut self, ui: &mut egui::Ui, effect: &mut VideoEffect) {
        let VideoEffect::SimpleText(SimpleText {
            text,
            position: [x, y],
            alignment,
            size,
            opacity,
        }) = effect
        else {
            return;
        };
        let typed = ui.add(
            egui::TextEdit::multiline(text)
                .desired_rows(2)
                .desired_width(f32::INFINITY),
        );
        // Typing shows live and lands in the history as one step when the field is left.
        if typed.changed() {
            self.output.change = self.output.change.max(Some(Change::Live));
        }
        if typed.lost_focus() {
            self.commit();
        }
        self.param(ui, "Position X", x, 0.0..=100.0, "%");
        self.param(ui, "Position Y", y, 0.0..=100.0, "%");
        self.options(
            ui,
            "Alignment",
            "simple_text_align",
            alignment,
            ALIGNMENTS,
            |align| match align {
                TextAlign::Left => "Left",
                TextAlign::Center => "Center",
                TextAlign::Right => "Right",
            },
        );
        self.param(ui, "Size", size, 1.0..=50.0, "%");
        self.param(ui, "Opacity", opacity, 0.0..=100.0, "%");
    }
}
