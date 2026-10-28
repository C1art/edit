use std::ops::RangeInclusive;

use egui::{DragValue, RichText};
use timeline::{BlendMode, Interpolation, Look, Param, Ticks};

use media::TrackDirection;

use super::color::ToneState;
use super::key_selection::KeySelection;
use super::lane::Lane;
use super::LutSlot;
use crate::color_match::ToneGoal;
use crate::mask_tracking::TrackSpan;
use crate::mixer::{response_state, Change};

pub(super) const ANIMATED: egui::Color32 = egui::Color32::from_rgb(90, 160, 240);

pub(super) struct Output {
    pub(super) change: Option<Change>,
    /// A keyframe to move the playhead to, in source time.
    pub(super) seek: Option<Ticks>,
    /// A LUT to ask for the file of.
    pub(super) choose_lut: Option<LutSlot>,
    /// Whether to analyze the clip's motion for Warp Stabilizer.
    pub(super) analyze_motion: bool,
    /// A mask to track through the footage from the playhead.
    pub(super) track_mask: Option<(usize, TrackDirection, TrackSpan)>,
    pub(super) stop_tracking: bool,
    /// What to set the Basic Correction for from the picture.
    pub(super) analyze_tone: Option<ToneGoal>,
    pub(super) keys: KeySelection,
}

/// Edits `look` in `rows`, of a clip among `video_tracks` video tracks.
pub(super) fn controls(
    ui: &mut egui::Ui,
    look: &mut Look,
    mut rows: Rows,
    video_tracks: usize,
) -> Output {
    egui::CollapsingHeader::new("Motion")
        .default_open(true)
        .show(ui, |ui| {
            let [x, y] = &mut look.motion.position;
            rows.param(ui, "Position X", x, -10_000.0..=10_000.0, "");
            rows.param(ui, "Position Y", y, -10_000.0..=10_000.0, "");
            let motion = &mut look.motion;
            if motion.uniform_scale {
                rows.param(ui, "Scale", &mut motion.scale, 0.0..=1_000.0, "%");
            } else {
                rows.param(ui, "Scale Height", &mut motion.scale, 0.0..=1_000.0, "%");
                let width = &mut motion.scale_width;
                rows.param(ui, "Scale Width", width, 0.0..=1_000.0, "%");
            }
            if ui
                .checkbox(&mut motion.uniform_scale, "Uniform Scale")
                .changed()
            {
                // Unlinking starts the width where the shared scale left it.
                if !motion.uniform_scale {
                    motion.scale_width = motion.scale.clone();
                }
                rows.commit();
            }
            rows.param(
                ui,
                "Rotation",
                &mut motion.rotation,
                -3_600.0..=3_600.0,
                "°",
            );
            let [x, y] = &mut motion.anchor;
            rows.param(ui, "Anchor X", x, -10_000.0..=10_000.0, "");
            rows.param(ui, "Anchor Y", y, -10_000.0..=10_000.0, "");
        });
    egui::CollapsingHeader::new("Opacity")
        .default_open(true)
        .show(ui, |ui| {
            rows.param(ui, "Opacity", &mut look.opacity, 0.0..=100.0, "%");
            ui.horizontal(|ui| {
                ui.label("Blend Mode");
                egui::ComboBox::from_id_salt("blend_mode")
                    .selected_text(look.blend.name())
                    .height(f32::INFINITY)
                    .show_ui(ui, |ui| {
                        for mode in BlendMode::ALL {
                            if mode.starts_group() {
                                ui.separator();
                            }
                            let choice = ui.selectable_value(&mut look.blend, mode, mode.name());
                            if choice.clicked() {
                                rows.commit();
                            }
                        }
                    });
            });
            super::masks::controls(ui, &mut rows, &mut look.masks);
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
                ui.push_id(index, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(effect.name()).strong());
                        if ui.small_button("Remove").clicked() {
                            removed = Some(index);
                        }
                    });
                    rows.effect(ui, effect, video_tracks);
                });
            }
            if let Some(index) = removed {
                look.effects.remove(index);
                rows.commit();
            }
            super::video_effects::add_menu(ui, &mut rows, &mut look.effects);
        });
    egui::CollapsingHeader::new("Color").show(ui, |ui| {
        super::color::controls(ui, &mut rows, &mut look.color);
    });
    ui.add_space(4.0);
    if ui.button("Reset Effects").clicked() && *look != Look::default() {
        *look = Look::default();
        rows.commit();
    }
    rows.output
}

/// Parameter rows at one source time, where animated parameters get their keyframes,
/// collecting what their edits ask for.
pub(super) struct Rows {
    time: Ticks,
    lane: Option<Lane>,
    /// How much of the clip's motion has been analyzed, while it is being.
    pub(super) analyzing: Option<f32>,
    /// Which mask is being tracked and how far along, while one is.
    pub(super) tracking: Option<(usize, f32)>,
    pub(super) tone: ToneState,
    pub(super) output: Output,
}

impl Rows {
    pub(super) fn new(time: Ticks, lane: Option<Lane>, keys: KeySelection) -> Self {
        Self {
            time,
            lane,
            analyzing: None,
            tracking: None,
            tone: ToneState::default(),
            output: Output {
                change: None,
                seek: None,
                choose_lut: None,
                analyze_motion: false,
                track_mask: None,
                stop_tracking: false,
                analyze_tone: None,
                keys,
            },
        }
    }

    pub(super) fn time(&self) -> Ticks {
        self.time
    }

    pub(super) fn commit(&mut self) {
        self.output.change = Some(Change::Commit);
    }

    /// A value with its keyframe controls: the first button turns animation on and off,
    /// the arrows jump between keyframes and the diamond adds or removes one here.
    pub(super) fn param(
        &mut self,
        ui: &mut egui::Ui,
        label: &str,
        param: &mut Param,
        range: RangeInclusive<f32>,
        suffix: &str,
    ) {
        let time = self.time;
        let graph = ui.make_persistent_id(("value graph", label));
        let mut open = ui.data(|data| data.get_temp(graph)).unwrap_or(false);
        ui.horizontal(|ui| {
            let animated = param.is_animated();
            let twirl = icon_button(
                ui,
                Icon::Twirl(open),
                false,
                self.lane.is_some() && animated,
            )
            .on_hover_text("Show the value graph");
            if twirl.clicked() {
                open = !open;
                ui.data_mut(|data| data.insert_temp(graph, open));
            }
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
                    .range(range.clone())
                    .max_decimals(2)
                    .suffix(suffix),
            );
            if response.changed() {
                param.set_at(time, value);
            }
            self.output.change = response_state(&response, self.output.change);
            if animated {
                self.key_buttons(ui, param);
            }
            if let Some(lane) = &self.lane {
                lane.show(ui, graph, param, time, &mut self.output);
            }
        });
        if let Some(lane) = self.lane.as_ref().filter(|_| open && param.is_animated()) {
            lane.show_graph(ui, param, &range, time, &mut self.output);
        }
    }

    /// Steps between keyframes and adds or removes the one at the playhead.
    fn key_buttons(&mut self, ui: &mut egui::Ui, param: &mut Param) {
        let time = self.time;
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let previous = param.previous_key(time);
            let back = icon_button(ui, Icon::Previous, false, previous.is_some())
                .on_hover_text("Previous keyframe");
            if back.clicked() {
                self.output.seek = previous;
            }
            let interpolation = param.interpolation_at(time);
            let shape = Icon::Key(interpolation.unwrap_or_default());
            let key = icon_button(ui, shape, interpolation.is_some(), true)
                .on_hover_text("Add or remove keyframe; right-click to set its interpolation");
            if key.clicked() {
                param.toggle_key(time);
                self.commit();
            }
            if let Some(current) = interpolation {
                key.context_menu(|ui| {
                    for choice in Interpolation::ALL {
                        if ui.radio(current == choice, choice.name()).clicked() {
                            param.set_interpolation(time, choice);
                            self.commit();
                            ui.close();
                        }
                    }
                });
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
pub(super) enum Icon {
    /// Points down when what it discloses is open.
    Twirl(bool),
    Stopwatch,
    Previous,
    Key(Interpolation),
    Next,
    /// A triangle the way it tracks, with a bar after it for a single frame.
    Track(TrackDirection, TrackSpan),
}

/// A small painted button; the default fonts have no glyphs for these symbols.
pub(super) fn icon_button(
    ui: &mut egui::Ui,
    icon: Icon,
    filled: bool,
    enabled: bool,
) -> egui::Response {
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
        Icon::Twirl(open) => {
            let points: &[(f32, f32)] = if open {
                &[(-4.0, -2.0), (4.0, -2.0), (0.0, 3.0)]
            } else {
                &[(-2.0, -4.0), (-2.0, 4.0), (3.0, 0.0)]
            };
            painter.add(egui::Shape::convex_polygon(at(points), color, stroke));
        }
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
        Icon::Key(interpolation) => paint_key(painter, c, interpolation, fill, stroke),
        Icon::Track(direction, span) => {
            let way = match direction {
                TrackDirection::Forward => 1.0,
                TrackDirection::Backward => -1.0,
            };
            let back = if span == TrackSpan::OneFrame {
                -1.5
            } else {
                0.0
            };
            let triangle =
                [(-3.0, -4.0), (-3.0, 4.0), (3.0, 0.0)].map(|(x, y)| (way * (x + back), y));
            painter.add(egui::Shape::convex_polygon(at(&triangle), color, stroke));
            if span == TrackSpan::OneFrame {
                let x = way * 4.5;
                painter.line_segment([c + egui::vec2(x, -4.0), c + egui::vec2(x, 4.0)], stroke);
            }
        }
    }
    response
}

/// A diamond for linear keyframes, a square for holds and a circle for eases.
pub(super) fn paint_key(
    painter: &egui::Painter,
    center: egui::Pos2,
    interpolation: Interpolation,
    fill: egui::Color32,
    stroke: egui::Stroke,
) {
    let at = |points: &[(f32, f32)]| {
        points
            .iter()
            .map(|&(x, y)| center + egui::vec2(x, y))
            .collect()
    };
    match interpolation {
        Interpolation::Linear => {
            painter.add(egui::Shape::convex_polygon(
                at(&[(0.0, -5.0), (5.0, 0.0), (0.0, 5.0), (-5.0, 0.0)]),
                fill,
                stroke,
            ));
        }
        Interpolation::Hold => {
            painter.add(egui::Shape::convex_polygon(
                at(&[(-4.0, -4.0), (4.0, -4.0), (4.0, 4.0), (-4.0, 4.0)]),
                fill,
                stroke,
            ));
        }
        _ => {
            painter.circle(center, 4.5, fill, stroke);
        }
    }
}
