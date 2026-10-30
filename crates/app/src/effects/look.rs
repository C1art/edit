use std::ops::RangeInclusive;

use egui::{DragValue, RichText};
use timeline::{BlendMode, Interpolation, Look, MaskOwner, MaskRef, Param, Ticks};

use media::TrackDirection;

use super::color::ToneState;
use super::key_icons::{icon_button, Icon};
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
    /// The effect, by index, whose Image Matte Key to ask for the picture of.
    pub(super) choose_matte: Option<usize>,
    /// Whether to analyze the clip's motion for Warp Stabilizer.
    pub(super) analyze_motion: bool,
    /// Whether to find the clip's subject for Auto Reframe.
    pub(super) analyze_reframe: bool,
    /// A mask to track through the footage from the playhead.
    pub(super) track_mask: Option<(MaskRef, TrackDirection, TrackSpan)>,
    pub(super) stop_tracking: bool,
    /// What to set the Basic Correction for from the picture.
    pub(super) analyze_tone: Option<ToneGoal>,
    pub(super) keys: KeySelection,
    /// Ken Burns boxes turned on or off.
    pub(super) ken_burns: Option<bool>,
    pub(super) swap_ken_burns: bool,
    /// Whether the Crop section is open.
    pub(super) cropping: bool,
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
            // Unlinking starts the width where the shared scale left it.
            if (!rows.simple || !motion.uniform_scale)
                && rows.checkbox(ui, &mut motion.uniform_scale, "Uniform Scale")
                && !motion.uniform_scale
            {
                motion.scale_width = motion.scale.clone();
            }
            rows.param(
                ui,
                "Rotation",
                &mut motion.rotation,
                -3_600.0..=3_600.0,
                "°",
            );
            let anchored = motion.anchor.iter().any(Param::is_animated)
                || motion
                    .anchor
                    .iter()
                    .any(|param| param.value_at(Ticks::ZERO) != 0.0);
            if !rows.simple || anchored {
                let [x, y] = &mut motion.anchor;
                rows.param(ui, "Anchor X", x, -10_000.0..=10_000.0, "");
                rows.param(ui, "Anchor Y", y, -10_000.0..=10_000.0, "");
            }
            ui.horizontal(|ui| {
                let on = rows.ken_burns;
                let toggle = ui.selectable_label(on, "Ken Burns").on_hover_text(
                    "Drag where the picture starts and ends over it in the source monitor",
                );
                if toggle.clicked() {
                    rows.output.ken_burns = Some(!on);
                }
                if on && ui.button("Swap Start and End").clicked() {
                    rows.output.swap_ken_burns = true;
                }
            });
        });
    egui::CollapsingHeader::new("Opacity")
        .default_open(true)
        .show(ui, |ui| {
            rows.param(ui, "Opacity", &mut look.opacity, 0.0..=100.0, "%");
            let blend = &mut look.blend;
            let plain = rows.simple && *blend == BlendMode::Normal;
            if !plain {
                rows.choice(ui, "Blend Mode", |ui| {
                    let mut chosen = false;
                    egui::ComboBox::from_id_salt("blend_mode")
                        .width(ui.available_width().min(ui.spacing().combo_width))
                        .selected_text(blend.name())
                        .height(f32::INFINITY)
                        .show_ui(ui, |ui| {
                            for mode in BlendMode::ALL {
                                if mode.starts_group() {
                                    ui.separator();
                                }
                                chosen |= ui.selectable_value(blend, mode, mode.name()).clicked();
                            }
                        });
                    chosen
                });
            }
            if !rows.simple || !look.masks.is_empty() {
                super::masks::controls(ui, &mut rows, MaskOwner::Opacity, &mut look.masks);
            }
        });
    let crop = egui::CollapsingHeader::new("Crop").show(ui, |ui| {
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
    rows.output.cropping = crop.fully_open();
    egui::CollapsingHeader::new("Effects")
        .default_open(true)
        .show(ui, |ui| {
            super::effect_stack::controls(ui, &mut rows, look, video_tracks);
        });
    // Simple mode keeps color to the Lumetri Color tab.
    if !rows.simple {
        egui::CollapsingHeader::new("Color").show(ui, |ui| {
            super::color::controls(ui, &mut rows, &mut look.color);
        });
    }
    ui.add_space(4.0);
    let changed = *look != Look::default();
    if ui
        .add_enabled(changed, egui::Button::new("Reset Effects"))
        .clicked()
    {
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
    /// How much of the clip has been analyzed for Auto Reframe, while it is being.
    pub(super) reframing: Option<f32>,
    /// Which mask is being tracked and how far along, while one is.
    pub(super) tracking: Option<(MaskRef, f32)>,
    pub(super) tone: ToneState,
    /// Whether the effect being shown asked for its matte picture.
    pub(super) browse_matte: bool,
    pub(super) ken_burns: bool,
    /// Simple mode: controls a first edit doesn't need stay hidden until they're used.
    pub(super) simple: bool,
    pub(super) output: Output,
}

impl Rows {
    pub(super) fn new(time: Ticks, lane: Option<Lane>, keys: KeySelection) -> Self {
        Self {
            time,
            lane,
            analyzing: None,
            reframing: None,
            tracking: None,
            tone: ToneState::default(),
            browse_matte: false,
            ken_burns: false,
            simple: false,
            output: Output {
                change: None,
                seek: None,
                choose_lut: None,
                choose_matte: None,
                analyze_motion: false,
                analyze_reframe: false,
                track_mask: None,
                stop_tracking: false,
                analyze_tone: None,
                keys,
                ken_burns: None,
                swap_ken_burns: false,
                cropping: false,
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
        self.param_text(ui, label, param, range, suffix, None);
    }

    /// A value shown as `shown` says, as a plugin writes its own, with `range` steps.
    pub(super) fn param_shown(
        &mut self,
        ui: &mut egui::Ui,
        label: &str,
        param: &mut Param,
        range: RangeInclusive<f32>,
        shown: Option<&str>,
    ) {
        self.param_text(ui, label, param, range, "", shown);
    }

    fn param_text(
        &mut self,
        ui: &mut egui::Ui,
        label: &str,
        param: &mut Param,
        range: RangeInclusive<f32>,
        suffix: &str,
        shown: Option<&str>,
    ) {
        let time = self.time;
        let graph = ui.make_persistent_id(("value graph", label));
        let mut open = ui.data(|data| data.get_temp(graph)).unwrap_or(false);
        if self.output.keys.toggles(graph) {
            if param.is_animated() {
                param.toggle_key(time);
            } else {
                param.toggle_animation(time);
            }
            self.commit();
        }
        ui.horizontal(|ui| {
            let animated = param.is_animated();
            // The graph only shows with the lane, which a narrow panel has no room for.
            if self.lane.is_some() {
                let twirl = icon_button(ui, Icon::Twirl(open), false, animated)
                    .on_hover_text("Show the value graph");
                if twirl.clicked() {
                    open = !open;
                    ui.data_mut(|data| data.insert_temp(graph, open));
                }
            }
            let stopwatch =
                icon_button(ui, Icon::Stopwatch, animated, true).on_hover_text("Toggle animation");
            if stopwatch.clicked() {
                param.toggle_animation(time);
                self.commit();
            }
            let text = RichText::new(label);
            let touched = stopwatch.clicked();
            let text = if animated { text.color(ANIMATED) } else { text };
            ui.add_sized([72.0, 18.0], egui::Label::new(text));
            let mut value = param.value_at(time);
            let mut drag = DragValue::new(&mut value)
                .speed(drag_speed(&range))
                .range(range.clone())
                .max_decimals(2)
                .suffix(suffix);
            if let Some(shown) = shown {
                drag = drag
                    .max_decimals(3)
                    .custom_formatter(|_, _| shown.to_owned())
                    .custom_parser(|text| text.trim().parse().ok());
            }
            let response = ui.add(drag);
            if response.changed() {
                param.set_at(time, value);
            }
            self.output.change = response_state(&response, self.output.change);
            let keyed = animated && self.key_buttons(ui, param);
            if touched || keyed || response.changed() || response.clicked() {
                self.output.keys.touch(graph);
            }
            if let Some(lane) = &self.lane {
                lane.show(ui, graph, param, time, &mut self.output);
            }
        });
        if let Some(lane) = self.lane.as_ref().filter(|_| open && param.is_animated()) {
            lane.show_graph(ui, param, &range, time, &mut self.output);
            lane.show_velocity(ui, param, time, &mut self.output);
        }
    }

    /// Steps between keyframes and adds or removes the one at the playhead; whether
    /// one of them was clicked.
    fn key_buttons(&mut self, ui: &mut egui::Ui, param: &mut Param) -> bool {
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
            back.clicked() || key.clicked() || forward.clicked()
        })
        .inner
    }
}

/// Units per dragged pixel: about a pixel each for wide ranges, finer for narrow ones.
fn drag_speed(range: &RangeInclusive<f32>) -> f32 {
    ((range.end() - range.start()) / 400.0).clamp(0.01, 1.0)
}
