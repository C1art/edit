use egui::{ComboBox, DragValue, Key};
use timeline::{FrameRate, InterpretFootage, SourceId};

use crate::editor::Editor;

/// Rates footage is commonly shot or delivered at.
const COMMON_RATES: [(&str, u32, u32); 10] = [
    ("23.976", 24_000, 1_001),
    ("24", 24, 1),
    ("25", 25, 1),
    ("29.97", 30_000, 1_001),
    ("30", 30, 1),
    ("48", 48, 1),
    ("50", 50, 1),
    ("59.94", 60_000, 1_001),
    ("60", 60, 1),
    ("120", 120, 1),
];

/// The frame rate an item's pictures play at, chosen in Premiere's Interpret Footage
/// dialog: the file's own, or one assumed in its place.
pub(crate) struct InterpretDialog {
    source: SourceId,
    name: String,
    native: FrameRate,
    assume: bool,
    rate: FrameRate,
}

impl Editor {
    pub(crate) fn open_interpret_footage(&mut self, source: SourceId) {
        let Some(item) = self.timeline.project().media_item(source) else {
            return;
        };
        let Some(video) = item.video.filter(|_| item.is_file()) else {
            self.notice("Only video files can be interpreted");
            return;
        };
        self.interpret_dialog = Some(InterpretDialog {
            source,
            name: item.name.clone(),
            native: video.frame_rate,
            assume: item.interpreted_rate.is_some(),
            rate: item.interpreted_rate.unwrap_or(video.frame_rate),
        });
    }
}

pub(crate) fn dialog(editor: &mut Editor, ctx: &egui::Context) {
    let Some(dialog) = &mut editor.interpret_dialog else {
        return;
    };
    let mut done = false;
    let mut cancelled = false;
    egui::Window::new("Interpret Footage")
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.strong(&dialog.name);
            let native = format!(
                "Use frame rate from file: {} fps",
                fps(dialog.native.approx_fps())
            );
            ui.radio_value(&mut dialog.assume, false, native);
            ui.horizontal(|ui| {
                ui.radio_value(&mut dialog.assume, true, "Assume this frame rate:");
                ui.add_enabled_ui(dialog.assume, |ui| rate_picker(ui, &mut dialog.rate));
            });
            ui.weak("Clips keep their frames, so they lengthen or shorten, and the sound follows.");
            ui.horizontal(|ui| {
                done = ui.button("OK").clicked() || ui.input(|input| input.key_pressed(Key::Enter));
                cancelled = ui.button("Cancel").clicked()
                    || ui.input(|input| input.key_pressed(Key::Escape));
            });
        });
    if !(done || cancelled) {
        return;
    }
    let Some(dialog) = editor.interpret_dialog.take() else {
        return;
    };
    if done {
        editor.interpret_footage(InterpretFootage {
            source: dialog.source,
            rate: dialog.assume.then_some(dialog.rate),
        });
    }
}

fn rate_picker(ui: &mut egui::Ui, rate: &mut FrameRate) {
    let shown = COMMON_RATES
        .iter()
        .find(|&&(_, num, den)| FrameRate::new(num, den) == Some(*rate))
        .map_or("Custom", |(name, ..)| name);
    ComboBox::from_id_salt("interpreted rate")
        .selected_text(shown)
        .show_ui(ui, |ui| {
            for (name, num, den) in COMMON_RATES {
                if let Some(common) = FrameRate::new(num, den) {
                    ui.selectable_value(rate, common, name);
                }
            }
        });
    let mut whole = rate.approx_fps().round() as u32;
    let custom = DragValue::new(&mut whole).range(1..=480).suffix(" fps");
    if ui.add(custom).changed() {
        if let Some(chosen) = FrameRate::new(whole, 1) {
            *rate = chosen;
        }
    }
}

/// Up to three decimals, without trailing zeros.
fn fps(rate: f64) -> String {
    let text = format!("{rate:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}
