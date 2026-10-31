use egui::Rect;
use timeline::Ticks;

use super::TimelineView;

/// How far in from either side of the tracks the view starts to scroll under a drag.
const ZONE: f32 = 24.0;
/// Points the view scrolls each second for every point the pointer reaches into the zone.
const SPEED: f32 = 12.0;
const MAX_SPEED: f32 = 3000.0;

impl TimelineView {
    /// Scrolls the view along while a clip, the playhead or dropped media is dragged
    /// toward or past either side, so a drag can reach time that's out of view.
    pub(super) fn edge_scroll(&mut self, ui: &egui::Ui, tracks: Rect, ruler_id: egui::Id) {
        let ctx = ui.ctx();
        let (pointer, dt) = ui.input(|input| (input.pointer.latest_pos(), input.stable_dt));
        let Some(pointer) = pointer else {
            return;
        };
        // Media passing over on its way elsewhere only scrolls the view from inside the tracks.
        let dropping = egui::DragAndDrop::has_any_payload(ctx) && tracks.contains(pointer);
        if self.drag.is_none() && !ctx.is_being_dragged(ruler_id) && !dropping {
            return;
        }
        let reach = if pointer.x < tracks.left() + ZONE {
            pointer.x - (tracks.left() + ZONE)
        } else if pointer.x > tracks.right() - ZONE {
            pointer.x - (tracks.right() - ZONE)
        } else {
            return;
        };
        if reach < 0.0 && self.scroll == Ticks::ZERO {
            return;
        }
        let points = (reach * SPEED).clamp(-MAX_SPEED, MAX_SPEED) * dt.min(0.1);
        self.scroll = (self.scroll + self.scale(0.0).span(points)).max(Ticks::ZERO);
        ctx.request_repaint();
    }
}
