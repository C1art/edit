use egui::{CursorIcon, Response};
use timeline::Sequence;

use super::gesture::Gesture;
use super::layout::{self, Hit};
use super::scale::TimeScale;
use super::transition_grip;
use super::{TimelineView, Tool};

impl TimelineView {
    /// Shows what pressing would grab: a trim at a clip's edge, a cut to roll, a volume
    /// line, and so on, held for as long as the drag lasts.
    pub(super) fn show_cursor(
        &self,
        response: &Response,
        sequence: &Sequence,
        scale: &TimeScale,
        top: f32,
    ) {
        let cursor = match &self.drag {
            Some(drag) => Some(match drag.gesture {
                Gesture::Move { .. } => CursorIcon::Grabbing,
                Gesture::Roll { .. } => CursorIcon::ResizeColumn,
                Gesture::Volume { .. } => CursorIcon::ResizeVertical,
                Gesture::Trim { .. }
                | Gesture::Stretch { .. }
                | Gesture::Slip { .. }
                | Gesture::Slide { .. }
                | Gesture::TransitionLength(_) => CursorIcon::ResizeHorizontal,
            }),
            None => response
                .hover_pos()
                .and_then(|pos| self.hover_cursor(response, sequence, scale, top, pos)),
        };
        if let Some(cursor) = cursor {
            response.ctx.set_cursor_icon(cursor);
        }
    }

    fn hover_cursor(
        &self,
        response: &Response,
        sequence: &Sequence,
        scale: &TimeScale,
        top: f32,
        pos: egui::Pos2,
    ) -> Option<CursorIcon> {
        let rows = layout::rows(sequence, &self.heights, top);
        let hit = layout::hit(sequence, &rows, scale, pos);
        let tool = self.tool.on_timeline();
        if tool == Tool::Selection
            && matches!(hit, Hit::Body { .. })
            && transition_grip::grip_at(sequence, &rows, scale, pos).is_some()
        {
            return Some(CursorIcon::ResizeHorizontal);
        }
        if tool == Tool::Selection && self.band_hit(sequence, &rows, scale, hit, pos).is_some() {
            let command = response.ctx.input(|input| input.modifiers.command);
            // Ctrl-click adds or removes a key rather than dragging the line.
            return Some(if command {
                CursorIcon::PointingHand
            } else {
                CursorIcon::ResizeVertical
            });
        }
        match (tool, hit) {
            (Tool::Razor, Hit::Body { .. } | Hit::Edge { .. }) => Some(CursorIcon::Crosshair),
            (Tool::Roll, Hit::Edge { .. }) => Some(CursorIcon::ResizeColumn),
            (Tool::Selection | Tool::Ripple | Tool::RateStretch, Hit::Edge { .. })
            | (Tool::Slip | Tool::Slide, Hit::Body { .. }) => Some(CursorIcon::ResizeHorizontal),
            _ => None,
        }
    }
}
