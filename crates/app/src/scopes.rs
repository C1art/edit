use egui::{pos2, Align2, Color32, FontId, Rect, Sense, Stroke};

use crate::editor::Editor;

const GRATICULE_STEP: usize = 25;
const LABEL_WIDTH: f32 = 28.0;

/// The luma waveform of the program monitor's frame.
pub fn ui(editor: &Editor, ui: &mut egui::Ui) {
    ui.label("Luma Waveform");
    let Some(texture) = editor
        .program
        .as_ref()
        .and_then(|program| program.waveform())
    else {
        ui.weak("Scopes need the GPU renderer.");
        return;
    };
    let width = ui.available_width();
    let height = (width * 0.6).min(ui.available_height()).max(120.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), Sense::hover());
    let trace = Rect::from_min_max(pos2(rect.left() + LABEL_WIDTH, rect.top()), rect.max);
    let painter = ui.painter_at(rect);
    painter.rect_filled(trace, 0.0, Color32::BLACK);
    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    painter.image(texture, trace, uv, Color32::WHITE);

    let line = Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 220, 120, 60));
    let text = ui.visuals().weak_text_color();
    for percent in (0..=100).step_by(GRATICULE_STEP) {
        let y = trace.bottom() - trace.height() * percent as f32 / 100.0;
        painter.hline(trace.x_range(), y, line);
        let anchor = match percent {
            0 => Align2::RIGHT_BOTTOM,
            100 => Align2::RIGHT_TOP,
            _ => Align2::RIGHT_CENTER,
        };
        painter.text(
            pos2(trace.left() - 4.0, y),
            anchor,
            percent.to_string(),
            FontId::monospace(10.0),
            text,
        );
    }
}
