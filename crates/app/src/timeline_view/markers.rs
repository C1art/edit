use egui::{epaint::PathShape, pos2, Align2, Color32, FontId, Painter, Rect, Stroke};
use timeline::{Clip, MarkerColor, MarkerId, Sequence};

use super::scale::TimeScale;

/// How close to a marker, in points, a click must land to pick it.
const HIT_POINTS: f32 = 6.0;
const HALF_WIDTH: f32 = 5.0;
const IN_OUT: Color32 = Color32::from_rgb(170, 170, 190);

pub fn color(color: MarkerColor) -> Color32 {
    match color {
        MarkerColor::Green => Color32::from_rgb(90, 200, 90),
        MarkerColor::Red => Color32::from_rgb(225, 70, 70),
        MarkerColor::Purple => Color32::from_rgb(190, 100, 220),
        MarkerColor::Orange => Color32::from_rgb(240, 150, 50),
        MarkerColor::Yellow => Color32::from_rgb(235, 220, 70),
        MarkerColor::White => Color32::from_rgb(235, 235, 235),
        MarkerColor::Blue => Color32::from_rgb(80, 130, 240),
        MarkerColor::Cyan => Color32::from_rgb(70, 210, 220),
    }
}

/// Shades the range between the in and out marks across the ruler and the tracks, with
/// brackets at the marks on the ruler.
pub fn paint_in_out(
    painter: &Painter,
    ruler: Rect,
    bottom: f32,
    scale: &TimeScale,
    sequence: &Sequence,
) {
    let marks = sequence.marks;
    if !marks.is_set() {
        return;
    }
    let range = marks.range(sequence.duration().max(marks.mark_in.unwrap_or_default()));
    let (left, right) = (scale.x(range.start), scale.x(range.end));
    let shade = Rect::from_x_y_ranges(left..=right, ruler.top()..=bottom);
    painter.rect_filled(shade, 0.0, Color32::from_white_alpha(14));
    let band = Rect::from_x_y_ranges(left..=right, ruler.top()..=ruler.top() + 4.0);
    painter.rect_filled(band, 0.0, IN_OUT);
    let stroke = Stroke::new(1.5, IN_OUT);
    let (top, foot) = (ruler.top(), ruler.bottom());
    if marks.mark_in.is_some() {
        painter.line_segment([pos2(left, top), pos2(left, foot)], stroke);
        painter.line_segment([pos2(left, foot), pos2(left + 4.0, foot)], stroke);
    }
    if marks.mark_out.is_some() {
        painter.line_segment([pos2(right, top), pos2(right, foot)], stroke);
        painter.line_segment([pos2(right, foot), pos2(right - 4.0, foot)], stroke);
    }
}

/// Draws each marker as a flag hanging from the bottom half of the ruler, with its name.
pub fn paint(painter: &Painter, ruler: Rect, scale: &TimeScale, sequence: &Sequence) {
    let top = ruler.center().y - 2.0;
    let bottom = ruler.bottom() - 1.0;
    for marker in &sequence.markers {
        let x = scale.x(marker.at);
        if x < ruler.left() - 200.0 || x > ruler.right() + HALF_WIDTH {
            continue;
        }
        let fill = color(marker.color);
        let flag = vec![
            pos2(x - HALF_WIDTH, top),
            pos2(x + HALF_WIDTH, top),
            pos2(x + HALF_WIDTH, bottom - HALF_WIDTH),
            pos2(x, bottom),
            pos2(x - HALF_WIDTH, bottom - HALF_WIDTH),
        ];
        painter.add(PathShape::convex_polygon(
            flag,
            fill,
            Stroke::new(1.0, Color32::BLACK),
        ));
        if !marker.name.is_empty() {
            painter.text(
                pos2(x + HALF_WIDTH + 3.0, top),
                Align2::LEFT_TOP,
                &marker.name,
                FontId::proportional(10.0),
                fill,
            );
        }
    }
}

/// The marker nearest screen `x`, if one is within reach.
pub fn hit(scale: &TimeScale, sequence: &Sequence, x: f32) -> Option<MarkerId> {
    sequence
        .markers
        .iter()
        .map(|marker| (marker.id, (scale.x(marker.at) - x).abs()))
        .filter(|&(_, distance)| distance <= HIT_POINTS)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(id, _)| id)
}

/// Draws a clip's markers as small flags under its top edge, each with a faint line
/// down the clip.
pub fn paint_on_clip(painter: &Painter, rect: Rect, scale: &TimeScale, clip: &Clip) {
    let painter = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    let top = rect.top() + 1.0;
    let half = HALF_WIDTH - 1.0;
    for (position, marker) in clip.shown_markers() {
        let x = scale.x(position);
        let fill = color(marker.color);
        painter.line_segment(
            [pos2(x, top), pos2(x, rect.bottom() - 1.0)],
            Stroke::new(1.0, fill.gamma_multiply(0.6)),
        );
        let flag = vec![
            pos2(x - half, top),
            pos2(x + half, top),
            pos2(x + half, top + half),
            pos2(x, top + 2.0 * half),
            pos2(x - half, top + half),
        ];
        painter.add(PathShape::convex_polygon(
            flag,
            fill,
            Stroke::new(1.0, Color32::BLACK),
        ));
    }
}

/// The clip marker nearest screen `x`, if one is within reach.
pub fn clip_hit(scale: &TimeScale, clip: &Clip, x: f32) -> Option<MarkerId> {
    clip.shown_markers()
        .map(|(position, marker)| (marker.id, (scale.x(position) - x).abs()))
        .filter(|&(_, distance)| distance <= HIT_POINTS)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(id, _)| id)
}
