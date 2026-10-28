use std::ops::RangeInclusive;

use egui::{DragValue, RichText};
use timeline::{Look, Param, Ticks, VideoEffect};

use crate::mixer::{response_state, Change};

const DEFAULT_BLURRINESS: i32 = 1_000;
const ANIMATED: egui::Color32 = egui::Color32::from_rgb(90, 160, 240);

pub(super) struct Output {
    pub(super) change: Option<Change>,
    /// A keyframe to move the playhead to, in source time.
    pub(super) seek: Option<Ticks>,
}

/// Edits `look` at source time `time`, where animated parameters get their keyframes.
pub(super) fn controls(ui: &mut egui::Ui, look: &mut Look, time: Ticks) -> Output {
    let mut rows = Rows {
        time,
        output: Output {
            change: None,
            seek: None,
        },
    };
    egui::CollapsingHeader::new("Motion")
        .default_open(true)
        .show(ui, |ui| {
            let [x, y] = &mut look.motion.position;
            rows.param(ui, "Position X", x, -10_000.0..=10_000.0, "");
            rows.param(ui, "Position Y", y, -10_000.0..=10_000.0, "");
            let motion = &mut look.motion;
            rows.param(ui, "Scale", &mut motion.scale, 0.0..=1_000.0, "%");
            rows.param(
                ui,
                "Rotation",
                &mut motion.rotation,
                -3_600.0..=3_600.0,
                "°",
            );
        });
    egui::CollapsingHeader::new("Opacity")
        .default_open(true)
        .show(ui, |ui| {
            rows.param(ui, "Opacity", &mut look.opacity, 0.0..=100.0, "%");
        });
    egui::CollapsingHeader::new("Crop").show(ui, |ui| {
        let crop = &mut look.crop;
        for (label, side) in [
            ("Left", &mut crop.left),
            ("Top", &mut crop.top),
            ("Right", &mut crop.right),
            ("Bottom", &mut crop.bottom),
        ] {
            rows.param(ui, label, side, 0.0..=100.0, "%");
        }
    });
    egui::CollapsingHeader::new("Effects")
        .default_open(true)
        .show(ui, |ui| {
            let mut removed = None;
            for (index, effect) in look.effects.iter_mut().enumerate() {
                ui.push_id(index, |ui| match effect {
                    VideoEffect::GaussianBlur { blurriness } => {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Gaussian Blur").strong());
                            if ui.small_button("Remove").clicked() {
                                removed = Some(index);
                            }
                        });
                        rows.param(ui, "Blurriness", blurriness, 0.0..=500.0, "");
                    }
                });
            }
            if let Some(index) = removed {
                look.effects.remove(index);
                rows.commit();
            }
            ui.menu_button("Add Effect", |ui| {
                if ui.button("Gaussian Blur").clicked() {
                    look.effects.push(VideoEffect::GaussianBlur {
                        blurriness: Param::new(DEFAULT_BLURRINESS),
                    });
                    rows.commit();
                }
            });
        });
    egui::CollapsingHeader::new("Color").show(ui, |ui| {
        let color = &mut look.color;
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
        rows.param(ui, "Saturation", &mut color.saturation, 0.0..=200.0, "%");
    });
    ui.add_space(4.0);
    if ui.button("Reset Effects").clicked() && *look != Look::default() {
        *look = Look::default();
        rows.commit();
    }
    rows.output
}

struct Rows {
    time: Ticks,
    output: Output,
}

impl Rows {
    fn commit(&mut self) {
        self.output.change = Some(Change::Commit);
    }

    /// A value with its keyframe controls: the first button turns animation on and off,
    /// the arrows jump between keyframes and the diamond adds or removes one here.
    fn param(
        &mut self,
        ui: &mut egui::Ui,
        label: &str,
        param: &mut Param,
        range: RangeInclusive<f32>,
        suffix: &str,
    ) {
        let time = self.time;
        ui.horizontal(|ui| {
            let animated = param.is_animated();
            let stopwatch =
                icon_button(ui, Icon::Stopwatch, animated, true).on_hover_text("Toggle animation");
            if stopwatch.clicked() {
                param.toggle_animation(time);
                self.commit();
            }
            let text = RichText::new(label);
            let text = if animated { text.color(ANIMATED) } else { text };
            ui.add_sized([72.0, 18.0], egui::Label::new(text));
            let mut value = param.value_at(time);
            let response = ui.add(
                DragValue::new(&mut value)
                    .speed(drag_speed(&range))
                    .range(range)
                    .max_decimals(2)
                    .suffix(suffix),
            );
            if response.changed() {
                param.set_at(time, value);
            }
            self.output.change = response_state(&response, self.output.change);
            if !animated {
                return;
            }
            ui.spacing_mut().item_spacing.x = 2.0;
            let previous = param.previous_key(time);
            let back = icon_button(ui, Icon::Previous, false, previous.is_some())
                .on_hover_text("Previous keyframe");
            if back.clicked() {
                self.output.seek = previous;
            }
            let key = icon_button(ui, Icon::Key, param.has_key_at(time), true)
                .on_hover_text("Add or remove keyframe");
            if key.clicked() {
                param.toggle_key(time);
                self.commit();
            }
            let next = param.next_key(time);
            let forward =
                icon_button(ui, Icon::Next, false, next.is_some()).on_hover_text("Next keyframe");
            if forward.clicked() {
                self.output.seek = next;
            }
        });
    }
}

/// Units per dragged pixel: about a pixel each for wide ranges, finer for narrow ones.
fn drag_speed(range: &RangeInclusive<f32>) -> f32 {
    ((range.end() - range.start()) / 400.0).clamp(0.01, 1.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Icon {
    Stopwatch,
    Previous,
    Key,
    Next,
}

/// A small painted button; the default fonts have no glyphs for these symbols.
fn icon_button(ui: &mut egui::Ui, icon: Icon, filled: bool, enabled: bool) -> egui::Response {
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(16.0, 18.0), sense);
    let visuals = ui.style().interact(&response);
    let mut color = if filled {
        ANIMATED
    } else {
        visuals.fg_stroke.color
    };
    if !enabled {
        color = color.gamma_multiply(0.35);
    }
    let fill = if filled {
        color
    } else {
        egui::Color32::TRANSPARENT
    };
    let stroke = egui::Stroke::new(1.2, color);
    let c = rect.center();
    let at = |points: &[(f32, f32)]| points.iter().map(|&(x, y)| c + egui::vec2(x, y)).collect();
    let painter = ui.painter();
    match icon {
        Icon::Stopwatch => {
            painter.circle(c + egui::vec2(0.0, 1.0), 5.0, fill, stroke);
            painter.line_segment(
                [c + egui::vec2(0.0, -7.0), c + egui::vec2(0.0, -4.0)],
                stroke,
            );
        }
        Icon::Previous => {
            painter.add(egui::Shape::convex_polygon(
                at(&[(3.0, -4.0), (3.0, 4.0), (-3.0, 0.0)]),
                color,
                stroke,
            ));
        }
        Icon::Next => {
            painter.add(egui::Shape::convex_polygon(
                at(&[(-3.0, -4.0), (-3.0, 4.0), (3.0, 0.0)]),
                color,
                stroke,
            ));
        }
        Icon::Key => {
            painter.add(egui::Shape::convex_polygon(
                at(&[(0.0, -5.0), (5.0, 0.0), (0.0, 5.0), (-5.0, 0.0)]),
                fill,
                stroke,
            ));
        }
    }
    response
}
