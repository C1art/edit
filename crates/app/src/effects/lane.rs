use egui::{Color32, Id, Rect, Sense, Stroke};
use timeline::{Clip, FrameRate, Interpolation, Param, Sequence, Ticks, Timebase};

use super::look::{paint_key, Output, ANIMATED};
use crate::mixer::Change;

mod graph;
mod velocity;

/// Room left of the lanes for a parameter's name, value and keyframe buttons.
const CONTROLS_WIDTH: f32 = 290.0;
const MIN_WIDTH: f32 = 60.0;
const HEIGHT: f32 = 18.0;
/// How near the pointer must be to grab a keyframe, or to snap one to the playhead.
const REACH: f32 = 6.0;
const SELECTED: Color32 = Color32::from_rgb(250, 190, 60);

/// The clip's length drawn beside each parameter, as in a grading panel's timeline:
/// the playhead and the keyframes, which drag to retime and right-click to change.
#[derive(Debug, Clone)]
pub(super) struct Lane {
    clip: Clip,
    rate: FrameRate,
    timebase: Timebase,
    left: f32,
    right: f32,
}

impl Lane {
    /// Lines the lanes up across `ui`, if it is wide enough for them.
    pub(super) fn new(ui: &egui::Ui, sequence: &Sequence, clip: &Clip) -> Option<Self> {
        let area = ui.max_rect();
        let (left, right) = (area.left() + CONTROLS_WIDTH, area.right() - 8.0);
        (right - left >= MIN_WIDTH && clip.duration > Ticks::ZERO).then(|| Self {
            clip: clip.clone(),
            rate: sequence.format.frame_rate,
            timebase: sequence.timebase,
            left,
            right,
        })
    }

    /// Draws `param`'s lane at the end of its row, with the playhead at source `time`.
    pub(super) fn show(
        &self,
        ui: &mut egui::Ui,
        id: Id,
        param: &mut Param,
        time: Ticks,
        output: &mut Output,
    ) {
        let left = self.left.max(ui.cursor().left() + 4.0);
        if self.right - left < MIN_WIDTH {
            return;
        }
        ui.add_space(left - ui.cursor().left());
        let size = egui::vec2(self.right - left, HEIGHT);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let dragging = response.id;
        let menu = response.id.with("menu");
        let pointer = response.interact_pointer_pos();
        let keys = &mut output.keys;
        if keys.act(id, param, time) {
            output.change = Some(Change::Commit);
        }
        let keys = &mut output.keys;
        if response.is_pointer_button_down_on() || response.clicked() {
            keys.press();
        }
        let adding = ui.input(|input| input.modifiers.command || input.modifiers.shift);
        if response.drag_started() {
            let origin = ui.input(|input| input.pointer.press_origin());
            let grabbed = origin.and_then(|origin| self.near_key(rect, param, origin.x));
            if let Some(key) = grabbed.filter(|&key| !keys.is_selected(id, key)) {
                keys.pick(id, key, adding);
            }
            ui.data_mut(|data| data.insert_temp(dragging, grabbed));
        }
        let grabbed: Option<Ticks> = ui.data(|data| data.get_temp(dragging)).flatten();
        if let (Some(from), Some(pointer)) = (grabbed.filter(|_| response.dragged()), pointer) {
            let playhead = self.x_of(rect, time);
            let to = if (pointer.x - playhead).abs() <= REACH {
                time
            } else {
                self.source_at(rect, pointer.x)
            };
            // The parameter's other selected keyframes go along.
            let moving = output.keys.selected_in(id);
            if param.shift_keys(&moving, to - from) {
                output.keys.shift(id, to - from);
                ui.data_mut(|data| data.insert_temp(dragging, Some(to)));
                output.change = output.change.max(Some(Change::Live));
            }
        } else if let (true, Some(pointer)) = (response.dragged(), pointer) {
            output.seek = Some(self.source_at(rect, pointer.x));
        }
        if response.drag_stopped() {
            if grabbed.is_some() {
                output.change = Some(Change::Commit);
            }
            ui.data_mut(|data| data.remove::<Option<Ticks>>(dragging));
        }
        if let (true, Some(pointer)) = (response.clicked(), pointer) {
            let key = self.near_key(rect, param, pointer.x);
            match key {
                Some(key) => output.keys.pick(id, key, adding),
                None if !adding => output.keys.clear(),
                None => {}
            }
            output.seek = Some(key.unwrap_or_else(|| self.source_at(rect, pointer.x)));
        }
        if let (true, Some(pointer)) = (response.secondary_clicked(), pointer) {
            let key = self.near_key(rect, param, pointer.x);
            ui.data_mut(|data| data.insert_temp(menu, key));
        }
        let menu_key: Option<Ticks> = ui.data(|data| data.get_temp(menu)).flatten();
        if let Some(key) = menu_key {
            response.context_menu(|ui| {
                let current = param.interpolation_at(key);
                for choice in Interpolation::ALL {
                    if ui.radio(current == Some(choice), choice.name()).clicked() {
                        param.set_interpolation(key, choice);
                        output.change = Some(Change::Commit);
                        ui.close();
                    }
                }
                ui.separator();
                if ui.button("Delete Keyframe").clicked() {
                    param.toggle_key(key);
                    output.change = Some(Change::Commit);
                    ui.close();
                }
            });
        }
        self.paint(ui, rect, param, time, |key| {
            output.keys.is_selected(id, key)
        });
    }

    fn paint(
        &self,
        ui: &egui::Ui,
        rect: Rect,
        param: &Param,
        time: Ticks,
        selected: impl Fn(Ticks) -> bool,
    ) {
        let painter = ui.painter_at(rect.expand2(egui::vec2(REACH, 0.0)));
        let visuals = ui.visuals();
        painter.rect_filled(rect, 2.0, visuals.extreme_bg_color);
        let middle = rect.center().y;
        let line = Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color);
        if param.is_animated() {
            painter.hline(rect.x_range(), middle, line);
        }
        let playhead = self.x_of(rect, time);
        painter.vline(
            playhead,
            rect.y_range(),
            Stroke::new(1.0, Color32::from_rgb(230, 70, 70)),
        );
        let outline = Stroke::new(1.0, Color32::BLACK);
        for key in &param.keys {
            let x = self.x_of(rect, key.time);
            if !(rect.left() - REACH..=rect.right() + REACH).contains(&x) {
                continue;
            }
            let fill = if selected(key.time) {
                SELECTED
            } else if key.time == time {
                Color32::WHITE
            } else {
                ANIMATED
            };
            paint_key(
                &painter,
                egui::pos2(x, middle),
                key.interpolation,
                fill,
                outline,
            );
        }
    }

    /// The keyframe drawn nearest `x`, if it is within reach.
    fn near_key(&self, rect: Rect, param: &Param, x: f32) -> Option<Ticks> {
        let distance = |key: Ticks| (self.x_of(rect, key) - x).abs();
        param
            .keys
            .iter()
            .map(|key| key.time)
            .filter(|&key| distance(key) <= REACH)
            .min_by(|&a, &b| distance(a).total_cmp(&distance(b)))
    }

    fn x_of(&self, rect: Rect, source: Ticks) -> f32 {
        let offset = self.clip.position_of(source) - self.clip.start;
        rect.left() + rect.width() * (offset.0 as f64 / self.clip.duration.0 as f64) as f32
    }

    /// The source time playing at the start of the sequence frame at `x`.
    fn source_at(&self, rect: Rect, x: f32) -> Ticks {
        let fraction = f64::from(((x - rect.left()) / rect.width()).clamp(0.0, 1.0));
        let offset = Ticks((self.clip.duration.0 as f64 * fraction).round() as i64);
        let frame = self.rate.frame_at(self.clip.start + offset, self.timebase);
        let position = self.rate.frame_start(frame, self.timebase);
        let clip = &self.clip;
        clip.source_time(position.clamp(clip.start, clip.end() - Ticks(1)))
    }
}
