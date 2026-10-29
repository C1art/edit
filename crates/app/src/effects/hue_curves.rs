use egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2};
use timeline::{CurvePoint, HueCurve, HueCurves, CURVE_RANGE, HUE_CURVE_NEUTRAL};

use super::curves::{from_screen, interact, paint_grid, paint_points, sampled_line, set_point};
use super::look::Rows;

const MAX_WIDTH: f32 = 230.0;
const HEIGHT: f32 = 140.0;
/// How wide the band is that a first point bends, either side of it.
const FIRST_BEND: u16 = CURVE_RANGE / 8;
const SWATCHES: usize = 48;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Along {
    Hue,
    Luma,
    Saturation,
}

const CURVES: [(&str, Along); 5] = [
    ("Hue vs Sat", Along::Hue),
    ("Hue vs Hue", Along::Hue),
    ("Hue vs Luma", Along::Hue),
    ("Luma vs Sat", Along::Luma),
    ("Sat vs Sat", Along::Saturation),
];

/// Lumetri's Hue Saturation Curves: pick a curve, click to bend it there, drag a point to
/// move it, double-click a point to remove it.
pub(super) fn controls(ui: &mut egui::Ui, rows: &mut Rows, curves: &mut HueCurves) {
    let pick_id = ui.make_persistent_id("hue_curve_pick");
    let mut pick = ui.data(|data| data.get_temp::<usize>(pick_id).unwrap_or(0));
    ui.horizontal_wrapped(|ui| {
        for (index, (name, _)) in CURVES.iter().enumerate() {
            ui.selectable_value(&mut pick, index, *name);
        }
        let curve = picked(curves, pick);
        if ui
            .add_enabled(!curve.is_neutral(), egui::Button::new("Reset").small())
            .clicked()
        {
            *curve = HueCurve::default();
            rows.commit();
        }
    });
    ui.data_mut(|data| data.insert_temp(pick_id, pick));
    let size = Vec2::new(ui.available_width().min(MAX_WIDTH), HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let along = CURVES[pick].1;
    let curve = picked(curves, pick);
    if curve.points.is_empty() && response.clicked() {
        if let Some(pointer) = response.interact_pointer_pos() {
            first_bend(curve, from_screen(rect, pointer), along);
        }
        rows.commit();
    } else {
        let change = interact(ui, &response, rect, &mut curve.points, 0);
        rows.output.change = change.max(rows.output.change);
    }
    paint(ui, rect, curve, along);
}

fn picked(curves: &mut HueCurves, index: usize) -> &mut HueCurve {
    match index {
        1 => &mut curves.hue_vs_hue,
        2 => &mut curves.hue_vs_luma,
        3 => &mut curves.luma_vs_sat,
        4 => &mut curves.sat_vs_sat,
        _ => &mut curves.hue_vs_sat,
    }
}

/// The first click on a flat curve sets a point with neutral ones either side, so
/// dragging it bends only the colors near it.
fn first_bend(curve: &mut HueCurve, at: CurvePoint, along: Along) {
    let neutral = |input| CurvePoint {
        input,
        output: HUE_CURVE_NEUTRAL,
    };
    let (before, after) = match along {
        Along::Hue => (
            (at.input + CURVE_RANGE - FIRST_BEND) % CURVE_RANGE,
            (at.input + FIRST_BEND) % CURVE_RANGE,
        ),
        _ => (
            at.input.saturating_sub(FIRST_BEND),
            (at.input + FIRST_BEND).min(CURVE_RANGE),
        ),
    };
    for point in [neutral(before), neutral(at.input), neutral(after)] {
        set_point(&mut curve.points, point);
    }
}

fn paint(ui: &egui::Ui, rect: Rect, curve: &HueCurve, along: Along) {
    let painter = ui.painter_at(rect);
    paint_grid(&painter, rect);
    let strip = Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - 6.0), rect.max);
    let width = strip.width() / SWATCHES as f32;
    for index in 0..SWATCHES {
        let input = (index as f32 + 0.5) / SWATCHES as f32;
        let left = strip.left() + width * index as f32;
        let swatch = Rect::from_min_size(
            Pos2::new(left, strip.top()),
            Vec2::new(width + 0.5, strip.height()),
        );
        painter.rect_filled(swatch, 0.0, swatch_color(along, input));
    }
    let middle = rect.center().y;
    let neutral = Stroke::new(1.0, Color32::from_gray(80));
    painter.line_segment(
        [
            Pos2::new(rect.left(), middle),
            Pos2::new(rect.right(), middle),
        ],
        neutral,
    );
    let wraps = along == Along::Hue;
    let line = sampled_line(
        rect,
        |input| curve.offset(input, wraps) + 0.5,
        Stroke::new(1.5, Color32::from_gray(230)),
    );
    painter.add(line);
    paint_points(&painter, rect, &curve.points, Color32::from_gray(230));
}

/// The color a curve's input stands for, under the curve.
fn swatch_color(along: Along, input: f32) -> Color32 {
    let level = (input * 255.0).round() as u8;
    match along {
        Along::Hue => {
            let hue = egui::ecolor::Hsva::new(input, 0.8, 0.9, 1.0);
            Color32::from(hue)
        }
        Along::Luma => Color32::from_gray(level),
        Along::Saturation => Color32::from(egui::ecolor::Hsva::new(0.0, input, 0.8, 1.0)),
    }
}
