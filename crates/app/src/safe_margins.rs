use egui::{Color32, Rect, Stroke, StrokeKind, Vec2};

/// Share of the frame's width and height inside which action, then titles, stay
/// clear of a TV's overscan, as Premiere draws them.
const ACTION_SAFE: f32 = 0.9;
const TITLE_SAFE: f32 = 0.8;
const CROSS: f32 = 8.0;

/// Outlines the action and title safe areas of a `canvas`-sized frame fitted in `area`,
/// with a cross at its center.
pub fn paint(ui: &egui::Ui, area: Rect, canvas: Vec2) {
    let fit = (area.width() / canvas.x).min(area.height() / canvas.y);
    let frame = Rect::from_center_size(area.center(), canvas * fit);
    let painter = ui.painter_at(frame);
    let stroke = Stroke::new(1.0, Color32::from_white_alpha(170));
    for share in [ACTION_SAFE, TITLE_SAFE] {
        let safe = Rect::from_center_size(frame.center(), frame.size() * share);
        painter.rect_stroke(safe, 0.0, stroke, StrokeKind::Middle);
    }
    let center = frame.center();
    painter.line_segment([center - Vec2::X * CROSS, center + Vec2::X * CROSS], stroke);
    painter.line_segment([center - Vec2::Y * CROSS, center + Vec2::Y * CROSS], stroke);
}
