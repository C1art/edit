use std::path::PathBuf;

use egui::epaint::{Mesh, Vertex, WHITE_UV};
use egui::{Color32, Pos2, RichText, Sense, Stroke, Vec2};
use timeline::{ColorCorrection, ColorWheel};

use super::look::Rows;
use crate::mixer::response_state;

const WHEEL_SIZE: f32 = 78.0;
/// How far a wheel's rim pushes toward its hue, as a percentage.
const RIM: f32 = 100.0;
/// Segments around a wheel's rim.
const SEGMENTS: usize = 48;

/// Which of a clip's LUTs a file is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LutSlot {
    Input,
    Look,
}

pub(super) fn controls(ui: &mut egui::Ui, rows: &mut Rows, color: &mut ColorCorrection) {
    lut_row(ui, rows, "Input LUT", &mut color.input_lut, LutSlot::Input);
    ui.label(RichText::new("White Balance").weak());
    rows.param(
        ui,
        "Temperature",
        &mut color.temperature,
        -100.0..=100.0,
        "",
    );
    rows.param(ui, "Tint", &mut color.tint, -100.0..=100.0, "");
    ui.label(RichText::new("Tone").weak());
    rows.param(ui, "Exposure", &mut color.exposure, -5.0..=5.0, "");
    rows.param(ui, "Contrast", &mut color.contrast, -100.0..=100.0, "");
    rows.param(ui, "Highlights", &mut color.highlights, -100.0..=100.0, "");
    rows.param(ui, "Shadows", &mut color.shadows, -100.0..=100.0, "");
    rows.param(ui, "Whites", &mut color.whites, -100.0..=100.0, "");
    rows.param(ui, "Blacks", &mut color.blacks, -100.0..=100.0, "");
    ui.label(RichText::new("Creative").weak());
    lut_row(ui, rows, "Look", &mut color.look, LutSlot::Look);
    if color.look.is_some() {
        rows.param(ui, "Intensity", &mut color.look_intensity, 0.0..=100.0, "%");
    }
    rows.param(ui, "Vibrance", &mut color.vibrance, -100.0..=100.0, "");
    rows.param(ui, "Saturation", &mut color.saturation, 0.0..=200.0, "%");
    egui::CollapsingHeader::new("RGB Curves").show(ui, |ui| {
        super::curves::controls(ui, rows, &mut color.curves);
    });
    ui.label(RichText::new("Color Wheels").weak());
    let mut wheels = [
        ("Shadows", &mut color.shadow_wheel),
        ("Midtones", &mut color.midtone_wheel),
        ("Highlights", &mut color.highlight_wheel),
    ];
    ui.horizontal(|ui| {
        for (name, wheel) in &mut wheels {
            ui.vertical(|ui| {
                ui.label(*name);
                wheel_pad(ui, rows, wheel);
            });
        }
    });
    for (name, wheel) in wheels {
        ui.push_id(name, |ui| {
            rows.param(ui, name, &mut wheel.level, -100.0..=100.0, "");
            egui::CollapsingHeader::new(format!("{name} Hue")).show(ui, |ui| {
                rows.param(ui, "Blue", &mut wheel.blue, -100.0..=100.0, "");
                rows.param(ui, "Red", &mut wheel.red, -100.0..=100.0, "");
            });
        });
    }
    super::secondary::controls(ui, rows, &mut color.secondary);
}

fn lut_row(
    ui: &mut egui::Ui,
    rows: &mut Rows,
    label: &str,
    lut: &mut Option<PathBuf>,
    slot: LutSlot,
) {
    ui.horizontal(|ui| {
        ui.label(label);
        match lut.as_ref().and_then(|path| path.file_name()) {
            Some(name) => {
                let path = lut.as_ref().map(|path| path.display().to_string());
                ui.label(name.to_string_lossy())
                    .on_hover_text(path.unwrap_or_default());
            }
            None => {
                ui.label(RichText::new("None").weak());
            }
        }
        if ui.small_button("Browse…").clicked() {
            rows.output.choose_lut = Some(slot);
        }
        if lut.is_some() && ui.small_button("Remove").clicked() {
            *lut = None;
            rows.commit();
        }
    });
}

/// A disc of hues with a puck at the wheel's push toward blue (right) and red (up), as
/// on a vectorscope. Dragging moves the puck; double-clicking centres it.
pub(super) fn wheel_pad(ui: &mut egui::Ui, rows: &mut Rows, wheel: &mut ColorWheel) {
    let time = rows.time();
    let (rect, mut response) =
        ui.allocate_exact_size(Vec2::splat(WHEEL_SIZE), Sense::click_and_drag());
    let center = rect.center();
    let radius = WHEEL_SIZE / 2.0 - 2.0;
    if response.double_clicked() {
        wheel.blue.set_at(time, 0.0);
        wheel.red.set_at(time, 0.0);
        response.mark_changed();
    } else if let Some(pointer) = response
        .interact_pointer_pos()
        .filter(|_| response.dragged())
    {
        let offset = (pointer - center) / radius;
        let offset = offset / offset.length().max(1.0);
        wheel.blue.set_at(time, offset.x * RIM);
        wheel.red.set_at(time, -offset.y * RIM);
        response.mark_changed();
    }
    rows.output.change = response_state(&response, rows.output.change);
    let painter = ui.painter_at(rect);
    painter.add(disc(center, radius));
    let stroke = Stroke::new(1.0, ui.visuals().widgets.noninteractive.fg_stroke.color);
    painter.circle_stroke(center, radius, stroke);
    let mark = Stroke::new(1.0, Color32::from_gray(90));
    for axis in [Vec2::X, Vec2::Y] {
        painter.line_segment([center - axis * 4.0, center + axis * 4.0], mark);
    }
    let puck = Vec2::new(wheel.blue.value_at(time), -wheel.red.value_at(time)) / RIM;
    painter.circle(
        center + puck * radius,
        4.0,
        Color32::WHITE,
        Stroke::new(1.5, Color32::BLACK),
    );
}

fn disc(center: Pos2, radius: f32) -> Mesh {
    let mut mesh = Mesh::default();
    mesh.vertices.push(Vertex {
        pos: center,
        uv: WHITE_UV,
        color: Color32::from_gray(128),
    });
    for step in 0..=SEGMENTS {
        let angle = step as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        let (red, blue) = angle.sin_cos();
        mesh.vertices.push(Vertex {
            pos: center + Vec2::new(blue, -red) * radius,
            uv: WHITE_UV,
            color: hue(blue, red),
        });
    }
    for step in 1..=SEGMENTS as u32 {
        mesh.add_triangle(0, step, step + 1);
    }
    mesh
}

/// Mid grey pushed along BT.709's Cb and Cr, the directions the shader uses.
fn hue(blue: f32, red: f32) -> Color32 {
    let [cb, cr] = [blue * 0.3, red * 0.3];
    let channel = |value: f32| ((0.5 + value).clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgb(
        channel(1.5748 * cr),
        channel(-0.1873 * cb - 0.4681 * cr),
        channel(1.8556 * cb),
    )
}
