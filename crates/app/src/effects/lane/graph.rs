use std::ops::RangeInclusive;

use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke};
use timeline::{HandleSide, Param, Ticks};

use super::{Lane, REACH};
use crate::effects::key_icons::paint_key;
use crate::effects::look::{Output, ANIMATED};
use crate::mixer::Change;

const HEIGHT: f32 = 64.0;
/// Room above and below the curve so keyframes at the extremes stay whole.
const MARGIN: f32 = 7.0;

/// The values the graph spans, fitted to the curve and held while a keyframe is dragged
/// so the curve does not rescale under the pointer.
#[derive(Debug, Clone, Copy)]
struct Scale {
    low: f32,
    high: f32,
}

impl Scale {
    fn fit(values: impl Iterator<Item = f32>) -> Self {
        let (low, high) = values.fold((f32::MAX, f32::MIN), |(low, high), value| {
            (low.min(value), high.max(value))
        });
        let pad = ((high - low) * 0.1).max(1.0);
        Self {
            low: low - pad,
            high: high + pad,
        }
    }

    fn y_of(self, rect: Rect, value: f32) -> f32 {
        let fraction = (value - self.low) / (self.high - self.low);
        rect.bottom() - MARGIN - fraction * (rect.height() - 2.0 * MARGIN)
    }

    fn value_at(self, rect: Rect, y: f32) -> f32 {
        let fraction = (rect.bottom() - MARGIN - y) / (rect.height() - 2.0 * MARGIN);
        self.low + fraction * (self.high - self.low)
    }
}

/// What a drag in the graph moves: a keyframe's value, or the slope of one of its Bezier
/// handles, drawn `reach` from it.
#[derive(Debug, Clone, Copy)]
enum Grab {
    Key(Ticks),
    Handle {
        key: Ticks,
        side: HandleSide,
        reach: Ticks,
    },
}

/// A Bezier handle as drawn, shortened if need be to stay inside the graph.
struct Handle {
    key: Ticks,
    side: HandleSide,
    reach: Ticks,
    from: Pos2,
    at: Pos2,
}

impl Lane {
    /// Premiere's value graph: the curve `param` takes over the clip, under its lane, where
    /// keyframes drag up and down to change their values and a double-click adds one.
    pub(in crate::effects) fn show_graph(
        &self,
        ui: &mut egui::Ui,
        param: &mut Param,
        range: &RangeInclusive<f32>,
        time: Ticks,
        output: &mut Output,
    ) {
        ui.horizontal(|ui| {
            let left = self.left.max(ui.cursor().left() + 4.0);
            ui.add_space(left - ui.cursor().left());
            let size = egui::vec2(self.right - left, HEIGHT);
            let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
            let dragging = response.id;
            let held: Option<(Grab, Scale)> = ui.data(|data| data.get_temp(dragging)).flatten();
            let scale = held.map_or_else(|| self.fit(rect, param), |(_, scale)| scale);
            let pointer = response.interact_pointer_pos();
            if response.drag_started() {
                let origin = ui.input(|input| input.pointer.press_origin());
                let grabbed = origin.and_then(|origin| self.grab_at(rect, scale, param, origin));
                let held = grabbed.map(|key| (key, scale));
                ui.data_mut(|data| data.insert_temp(dragging, held));
            }
            match (held.filter(|_| response.dragged()), pointer) {
                (Some((Grab::Handle { key, side, reach }, scale)), Some(pointer)) => {
                    if response.drag_delta() != egui::Vec2::ZERO {
                        let rise = scale.value_at(rect, pointer.y) - param.value_at(key);
                        let slope = f64::from(rise) * 100.0 / reach.0 as f64;
                        param.set_handle(key, side, slope);
                        output.change = output.change.max(Some(Change::Live));
                    }
                }
                (Some((Grab::Key(key), scale)), Some(pointer)) => {
                    let value = scale
                        .value_at(rect, pointer.y)
                        .clamp(*range.start(), *range.end());
                    if value != param.value_at(key) {
                        param.set_at(key, value);
                        output.change = output.change.max(Some(Change::Live));
                    }
                }
                (None, Some(pointer)) if response.dragged() => {
                    output.seek = Some(self.source_at(rect, pointer.x));
                }
                _ => {}
            }
            if response.drag_stopped() {
                if held.is_some() {
                    output.change = Some(Change::Commit);
                }
                ui.data_mut(|data| data.remove::<Option<(Grab, Scale)>>(dragging));
            }
            if let (true, Some(pointer)) = (response.double_clicked(), pointer) {
                let source = self.source_at(rect, pointer.x);
                if !param.has_key_at(source) {
                    param.set_at(source, param.value_at(source));
                    output.change = Some(Change::Commit);
                }
            } else if let (true, Some(pointer)) = (response.clicked(), pointer) {
                output.seek = Some(
                    self.key_at(rect, scale, param, pointer)
                        .unwrap_or_else(|| self.source_at(rect, pointer.x)),
                );
            }
            self.paint_graph(ui, rect, scale, param, time);
        });
    }

    fn fit(&self, rect: Rect, param: &Param) -> Scale {
        let columns = rect.width().max(1.0) as usize;
        let curve = (0..=columns)
            .map(|column| param.value_at(self.source_at(rect, rect.left() + column as f32)));
        Scale::fit(curve.chain(param.keys.iter().map(|key| key.value())))
    }

    /// The handle or else the keyframe drawn nearest `pointer`, if one is within reach.
    fn grab_at(&self, rect: Rect, scale: Scale, param: &Param, pointer: Pos2) -> Option<Grab> {
        let handle = self
            .handles(rect, scale, param)
            .into_iter()
            .map(|handle| (handle.at.distance(pointer), handle))
            .filter(|&(distance, _)| distance <= REACH)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, handle)| Grab::Handle {
                key: handle.key,
                side: handle.side,
                reach: handle.reach,
            });
        handle.or_else(|| self.key_at(rect, scale, param, pointer).map(Grab::Key))
    }

    /// Premiere draws a Bezier keyframe's handles a third of the way to its neighbours.
    fn handles(&self, rect: Rect, scale: Scale, param: &Param) -> Vec<Handle> {
        let keys = &param.keys;
        let bezier = keys.iter().enumerate().filter_map(|(index, key)| {
            let handles = key.handles.filter(|_| key.interpolation.has_handles())?;
            Some((index, key, handles))
        });
        bezier
            .flat_map(|(index, key, handles)| {
                let previous = index.checked_sub(1).and_then(|index| keys.get(index));
                let sides = [
                    (HandleSide::Incoming, previous),
                    (HandleSide::Outgoing, keys.get(index + 1)),
                ];
                sides.into_iter().filter_map(move |(side, neighbour)| {
                    let reach = (neighbour?.time - key.time).0 / 3;
                    let rise = handles.slope(side) * reach as f64 / 100.0;
                    let from = Pos2::new(self.x_of(rect, key.time), scale.y_of(rect, key.value()));
                    let y = scale.y_of(rect, key.value() + rise as f32);
                    let inside = y.clamp(rect.top() + MARGIN, rect.bottom() - MARGIN);
                    let shortened = if y == from.y {
                        1.0
                    } else {
                        f64::from((inside - from.y) / (y - from.y))
                    };
                    let reach = Ticks((reach as f64 * shortened).round() as i64);
                    (reach != Ticks::ZERO).then(|| Handle {
                        key: key.time,
                        side,
                        reach,
                        from,
                        at: Pos2::new(self.x_of(rect, key.time + reach), inside),
                    })
                })
            })
            .collect()
    }

    /// The keyframe drawn nearest `pointer`, if it is within reach.
    fn key_at(&self, rect: Rect, scale: Scale, param: &Param, pointer: Pos2) -> Option<Ticks> {
        let distance = |time: Ticks, value: f32| {
            Pos2::new(self.x_of(rect, time), scale.y_of(rect, value)).distance(pointer)
        };
        param
            .keys
            .iter()
            .map(|key| (key.time, distance(key.time, key.value())))
            .filter(|&(_, distance)| distance <= REACH * 1.5)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(time, _)| time)
    }

    fn paint_graph(&self, ui: &egui::Ui, rect: Rect, scale: Scale, param: &Param, time: Ticks) {
        let painter = ui.painter_at(rect.expand2(egui::vec2(REACH, 0.0)));
        let visuals = ui.visuals();
        painter.rect_filled(rect, 2.0, visuals.extreme_bg_color);
        let grid = Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color);
        let font = FontId::proportional(10.0);
        let weak = visuals.weak_text_color();
        for (value, anchor) in [
            (scale.high, Align2::LEFT_TOP),
            (scale.low, Align2::LEFT_BOTTOM),
        ] {
            let y = scale.y_of(rect, value);
            painter.hline(rect.x_range(), y, grid);
            let at = Pos2::new(rect.left() + 3.0, y);
            painter.text(at, anchor, format!("{value:.1}"), font.clone(), weak);
        }
        let columns = rect.width().max(1.0) as usize;
        let curve = (0..=columns)
            .map(|column| {
                let x = rect.left() + column as f32;
                let value = param.value_at(self.source_at(rect, x));
                Pos2::new(x, scale.y_of(rect, value))
            })
            .collect();
        painter.add(Shape::line(curve, Stroke::new(1.5, ANIMATED)));
        painter.vline(
            self.x_of(rect, time),
            rect.y_range(),
            Stroke::new(1.0, Color32::from_rgb(230, 70, 70)),
        );
        let outline = Stroke::new(1.0, Color32::BLACK);
        for handle in self.handles(rect, scale, param) {
            painter.line_segment([handle.from, handle.at], Stroke::new(1.0, ANIMATED));
            painter.circle(handle.at, 3.0, Color32::WHITE, outline);
        }
        for key in &param.keys {
            let x = self.x_of(rect, key.time);
            if !(rect.left() - REACH..=rect.right() + REACH).contains(&x) {
                continue;
            }
            let fill = if key.time == time {
                Color32::WHITE
            } else {
                ANIMATED
            };
            let center = Pos2::new(x, scale.y_of(rect, key.value()));
            paint_key(&painter, center, key.interpolation, fill, outline);
        }
    }
}
