use egui::ecolor::Hsva;
use egui::epaint::{Mesh, Vertex, WHITE_UV};
use egui::{Color32, RichText, Sense, Stroke, Vec2};
use timeline::HslSecondary;

use super::look::Rows;
use crate::mixer::response_state;

const STRIP_HEIGHT: f32 = 14.0;
const STRIP_STEPS: usize = 36;

pub(super) fn controls(ui: &mut egui::Ui, rows: &mut Rows, secondary: &mut HslSecondary) {
    egui::CollapsingHeader::new("HSL Secondary").show(ui, |ui| {
        ui.push_id("secondary", |ui| {
            if ui.checkbox(&mut secondary.enabled, "Enabled").changed() {
                rows.commit();
            }
            if secondary.enabled {
                key(ui, rows, secondary);
                correction(ui, rows, secondary);
            }
        });
    });
}

fn key(ui: &mut egui::Ui, rows: &mut Rows, secondary: &mut HslSecondary) {
    ui.label(RichText::new("Key").weak());
    let time = rows.time();
    ui.horizontal(|ui| {
        ui.label("Set Color");
        let [r, g, b, _] = hue_color(secondary.hue.value_at(time) / 360.0).to_array();
        let mut picked = [r, g, b];
        if ui.color_edit_button_srgb(&mut picked).changed() {
            let chosen = Hsva::from_srgb(picked);
            secondary.hue.set_at(time, chosen.h * 360.0);
            rows.commit();
        }
    });
    hue_strip(ui, rows, secondary);
    rows.param(ui, "Hue", &mut secondary.hue, 0.0..=360.0, "°");
    rows.param(ui, "Hue Width", &mut secondary.hue_width, 0.0..=180.0, "°");
    let [low, high] = &mut secondary.saturation_range;
    rows.param(ui, "Saturation Low", low, 0.0..=100.0, "%");
    rows.param(ui, "Saturation High", high, 0.0..=100.0, "%");
    let [low, high] = &mut secondary.lightness_range;
    rows.param(ui, "Lightness Low", low, 0.0..=100.0, "%");
    rows.param(ui, "Lightness High", high, 0.0..=100.0, "%");
    rows.param(ui, "Softness", &mut secondary.softness, 0.0..=100.0, "%");
    ui.horizontal(|ui| {
        let mut changed = ui
            .checkbox(&mut secondary.show_mask, "Show Mask")
            .on_hover_text("Keyed colors over grey")
            .changed();
        changed |= ui.checkbox(&mut secondary.inverted, "Invert").changed();
        if changed {
            rows.commit();
        }
    });
}

fn correction(ui: &mut egui::Ui, rows: &mut Rows, secondary: &mut HslSecondary) {
    ui.label(RichText::new("Correction").weak());
    super::color::wheel_pad(
        ui,
        rows,
        [&mut secondary.wheel.blue, &mut secondary.wheel.red],
        super::color::WHEEL_SIZE,
    );
    rows.param(ui, "Level", &mut secondary.wheel.level, -100.0..=100.0, "");
    rows.param(
        ui,
        "Temperature",
        &mut secondary.temperature,
        -100.0..=100.0,
        "",
    );
    rows.param(ui, "Tint", &mut secondary.tint, -100.0..=100.0, "");
    rows.param(ui, "Contrast", &mut secondary.contrast, -100.0..=100.0, "");
    rows.param(
        ui,
        "Saturation",
        &mut secondary.saturation,
        0.0..=200.0,
        "%",
    );
}

/// Every hue left to right with the keyed band outlined; clicking picks the hue.
fn hue_strip(ui: &mut egui::Ui, rows: &mut Rows, secondary: &mut HslSecondary) {
    let time = rows.time();
    let width = ui.available_width().min(220.0);
    let (rect, mut response) =
        ui.allocate_exact_size(Vec2::new(width, STRIP_HEIGHT), Sense::click_and_drag());
    if let Some(pointer) = response.interact_pointer_pos() {
        let fraction = ((pointer.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
        secondary.hue.set_at(time, fraction * 360.0);
        response.mark_changed();
    }
    rows.output.change = response_state(&response, rows.output.change);
    let painter = ui.painter_at(rect);
    let mut mesh = Mesh::default();
    for step in 0..=STRIP_STEPS {
        let fraction = step as f32 / STRIP_STEPS as f32;
        let x = rect.left() + fraction * rect.width();
        let color = hue_color(fraction);
        for y in [rect.top(), rect.bottom()] {
            mesh.vertices.push(Vertex {
                pos: egui::pos2(x, y),
                uv: WHITE_UV,
                color,
            });
        }
    }
    for step in 0..STRIP_STEPS as u32 {
        let at = step * 2;
        mesh.add_triangle(at, at + 1, at + 2);
        mesh.add_triangle(at + 1, at + 2, at + 3);
    }
    painter.add(mesh);
    let center = secondary.hue.value_at(time) / 360.0;
    let half = secondary.hue_width.value_at(time) / 360.0;
    let stroke = Stroke::new(1.5, Color32::WHITE);
    // The band may wrap past either end, so draw it shifted a turn each way too.
    for shift in [-1.0, 0.0, 1.0] {
        let [from, to] = [center - half + shift, center + half + shift]
            .map(|fraction| rect.left() + fraction * rect.width());
        if to < rect.left() || from > rect.right() {
            continue;
        }
        let band = egui::Rect::from_x_y_ranges(from..=to, rect.y_range());
        painter.rect_stroke(band, 0.0, stroke, egui::StrokeKind::Inside);
    }
    let x = rect.left() + center.rem_euclid(1.0) * rect.width();
    painter.vline(x, rect.y_range(), Stroke::new(1.0, Color32::BLACK));
}

fn hue_color(turns: f32) -> Color32 {
    Hsva::new(turns.rem_euclid(1.0), 1.0, 1.0, 1.0).into()
}
