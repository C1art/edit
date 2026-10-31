use std::path::PathBuf;

use egui::epaint::{Mesh, Vertex, WHITE_UV};
use egui::{Color32, Pos2, RichText, Sense, Stroke, Vec2};
use timeline::{ColorCorrection, ColorVignette, CreativeLook, Param};

use super::look::Rows;
use crate::color_match::ToneGoal;
use crate::mixer::response_state;

pub(super) const WHEEL_SIZE: f32 = 78.0;
const MIN_WHEEL_SIZE: f32 = 44.0;
/// How far a wheel's rim pushes toward its hue, as a percentage.
const RIM: f32 = 100.0;
/// Segments around a wheel's rim.
const SEGMENTS: usize = 48;

/// Whether Auto and Match can read the picture now.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) enum ToneState {
    #[default]
    Ready,
    /// The Comparison View is open, with a reference to match.
    ReadyToMatch,
    Analyzing,
}

/// Which of a clip's LUTs a file is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LutSlot {
    Input,
    Look,
}

pub(super) fn controls(ui: &mut egui::Ui, rows: &mut Rows, color: &mut ColorCorrection) {
    if !rows.simple || color.input_lut.is_some() {
        lut_row(ui, rows, "Input LUT", &mut color.input_lut, LutSlot::Input);
    }
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
    tone_buttons(ui, rows);
    rows.param(ui, "Exposure", &mut color.exposure, -5.0..=5.0, "");
    rows.param(ui, "Contrast", &mut color.contrast, -100.0..=100.0, "");
    rows.param(ui, "Highlights", &mut color.highlights, -100.0..=100.0, "");
    rows.param(ui, "Shadows", &mut color.shadows, -100.0..=100.0, "");
    rows.param(ui, "Whites", &mut color.whites, -100.0..=100.0, "");
    rows.param(ui, "Blacks", &mut color.blacks, -100.0..=100.0, "");
    ui.label(RichText::new("Creative").weak());
    look_presets(ui, rows, color);
    if !rows.simple {
        lut_row(ui, rows, "Look", &mut color.look, LutSlot::Look);
    }
    if color.look.is_some() {
        rows.param(ui, "Intensity", &mut color.look_intensity, 0.0..=100.0, "%");
    }
    rows.param(ui, "Faded Film", &mut color.faded_film, 0.0..=100.0, "");
    rows.param(ui, "Sharpen", &mut color.sharpen, 0.0..=100.0, "");
    rows.param(ui, "Vibrance", &mut color.vibrance, -100.0..=100.0, "");
    rows.param(ui, "Saturation", &mut color.saturation, 0.0..=200.0, "%");
    if !rows.simple {
        grading(ui, rows, color);
    }
    egui::CollapsingHeader::new("Vignette").show(ui, |ui| vignette(ui, rows, &mut color.vignette));
}

/// Curves, wheels and secondaries, for finer grading than a first edit needs.
fn grading(ui: &mut egui::Ui, rows: &mut Rows, color: &mut ColorCorrection) {
    split_tone(ui, rows, color);
    egui::CollapsingHeader::new("RGB Curves").show(ui, |ui| {
        super::curves::controls(ui, rows, &mut color.curves);
    });
    egui::CollapsingHeader::new("Hue Saturation Curves").show(ui, |ui| {
        super::hue_curves::controls(ui, rows, &mut color.hue_curves);
    });
    ui.label(RichText::new("Color Wheels").weak());
    let mut wheels = [
        ("Shadows", &mut color.shadow_wheel),
        ("Midtones", &mut color.midtone_wheel),
        ("Highlights", &mut color.highlight_wheel),
    ];
    let pads = wheels
        .each_mut()
        .map(|(name, wheel)| (*name, [&mut wheel.blue, &mut wheel.red]));
    wheel_row(ui, rows, pads);
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

fn look_presets(ui: &mut egui::Ui, rows: &mut Rows, color: &mut ColorCorrection) {
    let current = CreativeLook::of(color);
    let shown = match current {
        Some(Some(look)) => look.name(),
        Some(None) => "None",
        None => "Custom",
    };
    let mut picked = None;
    ui.horizontal(|ui| {
        ui.add_sized([72.0, 18.0], egui::Label::new("Preset"));
        egui::ComboBox::from_id_salt("creative look")
            .selected_text(shown)
            .height(320.0)
            .show_ui(ui, |ui| {
                let looks = [None].into_iter().chain(CreativeLook::ALL.map(Some));
                for look in looks {
                    let name = look.map_or("None", CreativeLook::name);
                    if ui.selectable_label(current == Some(look), name).clicked() {
                        picked = Some(look);
                    }
                }
            })
            .response
            .on_hover_text("Sets Faded Film, Vibrance, Saturation and the tints");
    });
    if let Some(look) = picked {
        CreativeLook::apply(look, color);
        rows.commit();
    }
}

fn split_tone(ui: &mut egui::Ui, rows: &mut Rows, color: &mut ColorCorrection) {
    let tints = [
        ("Shadow Tint", color.shadow_tint.each_mut()),
        ("Highlight Tint", color.highlight_tint.each_mut()),
    ];
    wheel_row(ui, rows, tints);
    rows.param(
        ui,
        "Tint Balance",
        &mut color.tint_balance,
        -100.0..=100.0,
        "",
    );
}

fn vignette(ui: &mut egui::Ui, rows: &mut Rows, vignette: &mut ColorVignette) {
    rows.param(ui, "Amount", &mut vignette.amount, -5.0..=5.0, "");
    rows.param(ui, "Midpoint", &mut vignette.midpoint, 0.0..=100.0, "");
    rows.param(ui, "Roundness", &mut vignette.roundness, -100.0..=100.0, "");
    rows.param(ui, "Feather", &mut vignette.feather, 0.0..=100.0, "");
}

fn tone_buttons(ui: &mut egui::Ui, rows: &mut Rows) {
    ui.horizontal(|ui| {
        let ready = rows.tone != ToneState::Analyzing;
        let auto = ui.add_enabled(ready, egui::Button::new("Auto"));
        if auto
            .on_hover_text("Set white balance and tone from the picture at the playhead")
            .clicked()
        {
            rows.output.analyze_tone = Some(ToneGoal::Auto);
        }
        let can_match = rows.tone == ToneState::ReadyToMatch;
        let matched = ui
            .add_enabled(can_match, egui::Button::new("Apply Match"))
            .on_hover_text("Match the Comparison View's reference frame")
            .on_disabled_hover_text("Open the Comparison View to choose a reference frame");
        if matched.clicked() {
            rows.output.analyze_tone = Some(ToneGoal::Match);
        }
        if !ready {
            ui.spinner();
        }
    });
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

/// Named wheels side by side, shrunk to share the width of a narrow panel.
fn wheel_row<const N: usize>(
    ui: &mut egui::Ui,
    rows: &mut Rows,
    wheels: [(&str, [&mut Param; 2]); N],
) {
    let gaps = ui.spacing().item_spacing.x * (N as f32 - 1.0);
    let size = ((ui.available_width() - gaps) / N as f32).clamp(MIN_WHEEL_SIZE, WHEEL_SIZE);
    ui.horizontal(|ui| {
        for (name, pad) in wheels {
            ui.vertical(|ui| {
                ui.set_width(size);
                ui.add(egui::Label::new(name).truncate());
                wheel_pad(ui, rows, pad, size);
            });
        }
    });
}

/// A disc of hues with a puck at a push toward blue (right) and red (up), as on a
/// vectorscope. Dragging moves the puck; double-clicking centres it.
pub(super) fn wheel_pad(
    ui: &mut egui::Ui,
    rows: &mut Rows,
    [blue, red]: [&mut Param; 2],
    size: f32,
) {
    let time = rows.time();
    let (rect, mut response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click_and_drag());
    let center = rect.center();
    let radius = size / 2.0 - 2.0;
    if response.double_clicked() {
        blue.set_at(time, 0.0);
        red.set_at(time, 0.0);
        response.mark_changed();
    } else if let Some(pointer) = response
        .interact_pointer_pos()
        .filter(|_| response.dragged())
    {
        let offset = (pointer - center) / radius;
        let offset = offset / offset.length().max(1.0);
        blue.set_at(time, offset.x * RIM);
        red.set_at(time, -offset.y * RIM);
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
    let puck = Vec2::new(blue.value_at(time), -red.value_at(time)) / RIM;
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
