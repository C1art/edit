use egui::{Color32, Rect, Sense, Stroke};
use playback::Player;

const RADIUS: f32 = 4.0;
const KEEPING_UP: Color32 = Color32::from_rgb(70, 200, 90);
const DROPPING: Color32 = Color32::from_rgb(230, 200, 60);

/// Premiere's dropped frame indicator: a light in the monitor's corner that turns yellow
/// once playing has had to skip frames to keep up, counting them on hover.
pub(crate) fn indicator(ui: &mut egui::Ui, area: Rect, player: &Player) {
    let center = area.left_top() + egui::vec2(10.0, 10.0);
    let dropped = player.dropped_frames();
    let color = if dropped == 0 { KEEPING_UP } else { DROPPING };
    let outline = Stroke::new(1.0, Color32::from_black_alpha(160));
    ui.painter().circle(center, RADIUS, color, outline);
    let hover = Rect::from_center_size(center, egui::Vec2::splat(4.0 * RADIUS));
    ui.interact(hover, ui.scope_id().with("dropped frames"), Sense::hover())
        .on_hover_text(format!("Dropped frames: {dropped}"));
}
