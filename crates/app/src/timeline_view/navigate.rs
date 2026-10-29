use egui::{CursorIcon, Rect, Response};
use timeline::{Sequence, Ticks};

use super::{layout, TimelineView, Tool};

impl TimelineView {
    /// Pans with the Hand tool and zooms with the Zoom tool, Alt zooming out. Returns
    /// whether the tool only navigates, so the tracks take no other gesture.
    pub(super) fn navigate(
        &mut self,
        response: &Response,
        tracks: Rect,
        sequence: &Sequence,
    ) -> bool {
        match self.tool {
            Tool::Hand => {
                let grabbing = response.dragged();
                let cursor = if grabbing {
                    CursorIcon::Grabbing
                } else {
                    CursorIcon::Grab
                };
                if response.hovered() || grabbing {
                    response.ctx.set_cursor_icon(cursor);
                }
                if grabbing {
                    let delta = response.drag_delta();
                    let scale = self.scale(0.0);
                    self.scroll = (self.scroll - scale.span(delta.x)).max(Ticks::ZERO);
                    let overflow =
                        (layout::total_height(sequence, &self.heights) - tracks.height() + 20.0)
                            .max(0.0);
                    self.scroll_y = (self.scroll_y - delta.y).clamp(0.0, overflow);
                }
                true
            }
            Tool::Zoom => {
                let out = response.ctx.input(|input| input.modifiers.alt);
                if response.hovered() {
                    let cursor = if out {
                        CursorIcon::ZoomOut
                    } else {
                        CursorIcon::ZoomIn
                    };
                    response.ctx.set_cursor_icon(cursor);
                }
                if let Some(pos) = response
                    .interact_pointer_pos()
                    .filter(|_| response.clicked())
                {
                    let factor = if out { 0.5 } else { 2.0 };
                    self.zoom(factor, Some(pos.x - tracks.left()));
                }
                true
            }
            _ => false,
        }
    }
}
