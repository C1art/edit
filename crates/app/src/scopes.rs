use egui::{pos2, vec2, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke};
use render::Scope;

use crate::editor::Editor;

const GRATICULE_STEP: usize = 25;
const LABEL_WIDTH: f32 = 28.0;
const GRATICULE: Color32 = Color32::from_rgba_premultiplied(60, 53, 29, 60);
/// The primaries and secondaries at 75% amplitude, as on broadcast color bars.
const TARGETS: [(&str, [f32; 3]); 6] = [
    ("R", [0.75, 0.0, 0.0]),
    ("Mg", [0.75, 0.0, 0.75]),
    ("B", [0.0, 0.0, 0.75]),
    ("Cy", [0.0, 0.75, 0.75]),
    ("G", [0.0, 0.75, 0.0]),
    ("Yl", [0.75, 0.75, 0.0]),
];
/// Where skin of any complexion falls, counterclockwise from the Cb axis.
const SKIN_TONE_DEGREES: f32 = 123.0;

/// Scopes of the program monitor's frame.
pub fn ui(editor: &mut Editor, ui: &mut egui::Ui) {
    let id = ui.scope_id().with("scope");
    let mut scope: Scope = ui.data(|data| data.get_temp(id)).unwrap_or_default();
    ui.horizontal(|ui| {
        for (kind, name) in [
            (Scope::LumaWaveform, "Luma Waveform"),
            (Scope::RgbParade, "RGB Parade"),
            (Scope::Vectorscope, "Vectorscope"),
            (Scope::Histogram, "Histogram"),
        ] {
            ui.selectable_value(&mut scope, kind, name);
        }
    });
    ui.data_mut(|data| data.insert_temp(id, scope));
    let Some(program) = &mut editor.program else {
        return;
    };
    program.set_scope(Some(scope));
    let Some(texture) = program.scope() else {
        ui.weak("Scopes need the GPU renderer.");
        return;
    };
    let width = ui.available_width();
    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    if scope == Scope::Vectorscope {
        let side = width.min(ui.available_height()).max(120.0);
        let (rect, _) = ui.allocate_exact_size(vec2(width, side), Sense::hover());
        let trace = Rect::from_center_size(rect.center(), vec2(side, side));
        let painter = ui.painter_at(rect);
        painter.image(texture, trace, uv, Color32::WHITE);
        vectorscope_graticule(&painter, trace, ui.visuals().weak_text_color());
        return;
    }
    let height = (width * 0.6).min(ui.available_height()).max(120.0);
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let trace = Rect::from_min_max(pos2(rect.left() + LABEL_WIDTH, rect.top()), rect.max);
    let painter = ui.painter_at(rect);
    painter.image(texture, trace, uv, Color32::WHITE);
    let line = Stroke::new(1.0, GRATICULE);
    if scope == Scope::Histogram {
        for quarter in 1..4 {
            let x = trace.left() + trace.width() * quarter as f32 / 4.0;
            painter.vline(x, trace.y_range(), line);
        }
    }
    if scope == Scope::RgbParade {
        for third in 1..3 {
            let x = trace.left() + trace.width() * third as f32 / 3.0;
            painter.vline(x, trace.y_range(), Stroke::new(1.0, Color32::from_gray(70)));
        }
    }
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

/// The edge of the legal range, the axes, the color bar targets and the skin tone line.
fn vectorscope_graticule(painter: &egui::Painter, trace: Rect, text: Color32) {
    let line = Stroke::new(1.0, GRATICULE);
    let center = trace.center();
    let radius = trace.width() / 2.0;
    painter.circle_stroke(center, radius, line);
    painter.hline(trace.x_range(), center.y, line);
    painter.vline(center.x, trace.y_range(), line);
    for (name, rgb) in TARGETS {
        let target = chroma_position(trace, rgb);
        painter.rect_stroke(
            Rect::from_center_size(target, vec2(8.0, 8.0)),
            0.0,
            Stroke::new(1.0, text),
            egui::StrokeKind::Middle,
        );
        let outward = (target - center).normalized() * 12.0;
        painter.text(
            target + outward,
            Align2::CENTER_CENTER,
            name,
            FontId::monospace(10.0),
            text,
        );
    }
    let angle = SKIN_TONE_DEGREES.to_radians();
    let skin = center + vec2(angle.cos(), -angle.sin()) * radius;
    painter.line_segment([center, skin], line);
}

/// Where a display-encoded color lands: Cb across, Cr up, as the scope bins it.
fn chroma_position(trace: Rect, [r, g, b]: [f32; 3]) -> Pos2 {
    let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let cb = (b - luma) / 1.8556;
    let cr = (r - luma) / 1.5748;
    trace.center() + vec2(cb, -cr) * trace.width()
}
