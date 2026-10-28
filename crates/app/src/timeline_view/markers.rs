use egui::{epaint::PathShape, pos2, Align2, Color32, FontId, Painter, Rect, Stroke};
use timeline::{MarkerColor, MarkerId, Sequence};

use super::scale::TimeScale;

/// How close to a marker, in points, a click must land to pick it.
const HIT_POINTS: f32 = 6.0;
const HALF_WIDTH: f32 = 5.0;

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
