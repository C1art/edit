use egui::{Color32, Pos2, Rect, Sense, Shape, Stroke, Vec2};
use timeline::{Curve, CurvePoint, Curves, CURVE_RANGE};

use super::look::Rows;
use crate::mixer::Change;

const MAX_SIZE: f32 = 230.0;
/// How near, in points, the pointer must be to grab a curve point.
const GRAB_RADIUS: f32 = 8.0;
const CURVE_STEPS: usize = 96;

const CHANNELS: [(&str, Color32); 4] = [
    ("Master", Color32::from_gray(230)),
    ("Red", Color32::from_rgb(235, 80, 80)),
    ("Green", Color32::from_rgb(80, 200, 100)),
    ("Blue", Color32::from_rgb(90, 140, 255)),
];

/// RGB curves: pick a channel, click to add a point, drag to move one, double-click a
/// point to remove it.
pub(super) fn controls(ui: &mut egui::Ui, rows: &mut Rows, curves: &mut Curves) {
    let channel_id = ui.make_persistent_id("curve_channel");
    let mut channel = ui.data(|data| data.get_temp::<usize>(channel_id).unwrap_or(0));
    ui.horizontal(|ui| {
        for (index, (name, _)) in CHANNELS.iter().enumerate() {
            ui.selectable_value(&mut channel, index, *name);
        }
        let curve = channel_curve(curves, channel);
        if ui
            .add_enabled(!curve.is_identity(), egui::Button::new("Reset").small())
            .clicked()
        {
            *curve = Curve::default();
            rows.commit();
        }
    });
    ui.data_mut(|data| data.insert_temp(channel_id, channel));
    let side = ui.available_width().min(MAX_SIZE);
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(side), Sense::click_and_drag());
    let color = CHANNELS[channel].1;
    let curve = channel_curve(curves, channel);
    let change = interact(ui, &response, rect, &mut curve.points, 2);
    rows.output.change = change.max(rows.output.change);
    paint(ui, rect, curves, channel, color);
}

fn channel_curve(curves: &mut Curves, channel: usize) -> &mut Curve {
    match channel {
        1 => &mut curves.red,
        2 => &mut curves.green,
        3 => &mut curves.blue,
        _ => &mut curves.master,
    }
}

/// Click adds a point, drag moves one and double-click removes one while more than
/// `keep` remain.
pub(super) fn interact(
    ui: &egui::Ui,
    response: &egui::Response,
    rect: Rect,
    points: &mut Vec<CurvePoint>,
    keep: usize,
) -> Option<Change> {
    let dragging_id = response.id.with("dragging");
    if response.drag_stopped() {
        ui.data_mut(|data| data.remove::<usize>(dragging_id));
        return Some(Change::Commit);
    }
    let pointer = response.interact_pointer_pos()?;
    let nearest = points
        .iter()
        .map(|&point| to_screen(rect, point).distance(pointer))
        .enumerate()
        .filter(|&(_, distance)| distance <= GRAB_RADIUS)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(index, _)| index);
    if response.double_clicked() {
        let index = nearest.filter(|_| points.len() > keep)?;
        points.remove(index);
        return Some(Change::Commit);
    }
    if response.drag_started() {
        let index = match nearest {
            Some(index) => index,
            None => {
                let point = from_screen(rect, pointer);
                set_point(points, point);
                points
                    .iter()
                    .position(|existing| existing.input == point.input)?
            }
        };
        ui.data_mut(|data| data.insert_temp(dragging_id, index));
    }
    if response.dragged() {
        let index = ui.data(|data| data.get_temp::<usize>(dragging_id))?;
        move_point(points, index, from_screen(rect, pointer));
        return Some(Change::Live);
    }
    if response.clicked() && nearest.is_none() {
        set_point(points, from_screen(rect, pointer));
        return Some(Change::Commit);
    }
    None
}

/// Adds a point, or moves the one already at its input, keeping the points sorted.
pub(super) fn set_point(points: &mut Vec<CurvePoint>, point: CurvePoint) {
    match points.binary_search_by_key(&point.input, |existing| existing.input) {
        Ok(index) => points[index] = point,
        Err(index) => points.insert(index, point),
    }
}

/// Moves a point, keeping it strictly between its neighbours so the points stay sorted.
fn move_point(points: &mut [CurvePoint], index: usize, to: CurvePoint) {
    let low = index
        .checked_sub(1)
        .and_then(|before| points.get(before))
        .map_or(0, |before| before.input + 1);
    let high = points
        .get(index + 1)
        .map_or(CURVE_RANGE, |after| after.input.saturating_sub(1));
    if let Some(point) = points.get_mut(index) {
        *point = CurvePoint {
            input: to.input.clamp(low, high.max(low)),
            output: to.output,
        };
    }
}

pub(super) fn to_screen(rect: Rect, point: CurvePoint) -> Pos2 {
    let range = f32::from(CURVE_RANGE);
    Pos2::new(
        rect.left() + rect.width() * f32::from(point.input) / range,
        rect.bottom() - rect.height() * f32::from(point.output) / range,
    )
}

pub(super) fn from_screen(rect: Rect, pos: Pos2) -> CurvePoint {
    let level = |fraction: f32| (fraction.clamp(0.0, 1.0) * f32::from(CURVE_RANGE)).round() as u16;
    CurvePoint {
        input: level((pos.x - rect.left()) / rect.width()),
        output: level((rect.bottom() - pos.y) / rect.height()),
    }
}

fn paint(ui: &egui::Ui, rect: Rect, curves: &mut Curves, channel: usize, color: Color32) {
    let painter = ui.painter_at(rect);
    paint_grid(&painter, rect);
    let grid = Stroke::new(1.0, Color32::from_gray(48));
    painter.line_segment([rect.left_bottom(), rect.right_top()], grid);
    // The other channels' curves show faintly, as they shape the result too.
    for (index, (_, other)) in CHANNELS.iter().enumerate() {
        let curve = channel_curve(curves, index);
        if index != channel && !curve.is_identity() {
            painter.add(curve_line(
                rect,
                curve,
                Stroke::new(1.0, other.gamma_multiply(0.4)),
            ));
        }
    }
    let curve = channel_curve(curves, channel);
    painter.add(curve_line(rect, curve, Stroke::new(1.5, color)));
    paint_points(&painter, rect, &curve.points, color);
}

pub(super) fn paint_points(
    painter: &egui::Painter,
    rect: Rect,
    points: &[CurvePoint],
    color: Color32,
) {
    for &point in points {
        painter.circle(
            to_screen(rect, point),
            3.5,
            color,
            Stroke::new(1.0, Color32::BLACK),
        );
    }
}

/// A dark square ruled in quarters.
pub(super) fn paint_grid(painter: &egui::Painter, rect: Rect) {
    painter.rect_filled(rect, 2.0, Color32::from_gray(24));
    let grid = Stroke::new(1.0, Color32::from_gray(48));
    for quarter in 1..4 {
        let fraction = quarter as f32 / 4.0;
        let x = rect.left() + rect.width() * fraction;
        let y = rect.top() + rect.height() * fraction;
        painter.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            grid,
        );
        painter.line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            grid,
        );
    }
}

fn curve_line(rect: Rect, curve: &Curve, stroke: Stroke) -> Shape {
    sampled_line(rect, |input| curve.sample(input), stroke)
}

/// The line through `output` at inputs across `rect`, both 0 to 1.
pub(super) fn sampled_line(rect: Rect, output: impl Fn(f32) -> f32, stroke: Stroke) -> Shape {
    let points = (0..=CURVE_STEPS)
        .map(|step| {
            let input = step as f32 / CURVE_STEPS as f32;
            let output = output(input);
            Pos2::new(
                rect.left() + rect.width() * input,
                rect.bottom() - rect.height() * output,
            )
        })
        .collect();
    Shape::line(points, stroke)
}
