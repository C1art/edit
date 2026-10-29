use egui::{Rect, Sense};
use timeline::{Sequence, Ticks};

use super::TimelineView;
use crate::theme;

/// Height of the bar under the tracks that scrolls and zooms the view along time.
pub(super) const HEIGHT: f32 = 12.0;
/// How far in from either end of the thumb a press grabs that end to zoom.
const END_GRAB: f32 = 8.0;
const MIN_THUMB: f32 = 2.0 * END_GRAB + 8.0;

/// What a drag on the bar holds, with the span of time the bar stood for when it began,
/// so the thumb keeps pace with the pointer even as that span grows.
#[derive(Debug, Clone, Copy)]
enum Grab {
    Thumb { total: Ticks },
    Start { total: Ticks },
    End { total: Ticks },
}

impl TimelineView {
    /// Premiere's zoom scroll bar: drag the thumb to scroll, drag its ends to zoom, click
    /// beside it to page and double-click to fit the sequence.
    pub(super) fn time_scrollbar(&mut self, ui: &egui::Ui, sequence: &Sequence, bar: Rect) {
        let visible = self.scale(0.0).span(self.width);
        // A little room past the end, so the last clip doesn't sit against the edge.
        let total = (sequence.duration() + Ticks(visible.0 / 10)).max(self.scroll + visible);
        let per_point = |total: Ticks| total.0 as f64 / f64::from(bar.width().max(1.0));
        let thumb_x = |time: Ticks| bar.left() + (time.0 as f64 / per_point(total)) as f32;
        let mut thumb = Rect::from_x_y_ranges(
            thumb_x(self.scroll)..=thumb_x(self.scroll + visible),
            bar.y_range(),
        );
        if thumb.width() < MIN_THUMB {
            thumb = Rect::from_center_size(thumb.center(), egui::vec2(MIN_THUMB, bar.height()))
                .intersect(bar);
        }

        let id = ui.scope_id().with("time scrollbar");
        let response = ui.interact(bar, id, Sense::click_and_drag());
        let grab_id = id.with("grab");
        if response.drag_started() {
            let origin = ui.input(|input| input.pointer.press_origin());
            let grab = origin.and_then(|pos| {
                if !thumb.contains(pos) {
                    None
                } else if pos.x < thumb.left() + END_GRAB {
                    Some(Grab::Start { total })
                } else if pos.x > thumb.right() - END_GRAB {
                    Some(Grab::End { total })
                } else {
                    Some(Grab::Thumb { total })
                }
            });
            ui.data_mut(|data| data.insert_temp(grab_id, grab));
        }
        let grab = ui
            .data(|data| data.get_temp::<Option<Grab>>(grab_id))
            .flatten();
        if response.double_clicked() {
            self.zoom_to_fit(sequence);
        } else if response.dragged() {
            let moved =
                |total| Ticks((f64::from(response.drag_delta().x) * per_point(total)) as i64);
            let end = self.scroll + visible;
            match grab {
                Some(Grab::Thumb { total }) => {
                    self.scroll = (self.scroll + moved(total)).max(Ticks::ZERO);
                }
                Some(Grab::Start { total }) => {
                    let start = (self.scroll + moved(total)).clamp(Ticks::ZERO, end);
                    self.show_span(start, end);
                }
                Some(Grab::End { total }) => {
                    let end = (end + moved(total)).max(self.scroll);
                    self.show_span(self.scroll, end);
                }
                None => {}
            }
        } else if let Some(pos) = response
            .interact_pointer_pos()
            .filter(|_| response.clicked())
        {
            let page = Ticks(visible.0 * 9 / 10);
            self.scroll = if pos.x < thumb.left() {
                (self.scroll - page).max(Ticks::ZERO)
            } else if pos.x > thumb.right() {
                self.scroll + page
            } else {
                self.scroll
            };
        }
        if response.drag_stopped() {
            ui.data_mut(|data| data.remove::<Option<Grab>>(grab_id));
        }

        let painter = ui.painter_at(bar);
        painter.rect_filled(bar, 0.0, ui.visuals().extreme_bg_color.gamma_multiply(0.6));
        let thumb = thumb.shrink2(egui::vec2(1.0, 2.0));
        painter.rect_filled(thumb, thumb.height() / 2.0, theme::thumb_fill(&response));
        // Dots at either end mark where the thumb is grabbed to zoom.
        let dot = theme::TEXT.gamma_multiply(0.7);
        for x in [
            thumb.left() + END_GRAB / 2.0 + 1.0,
            thumb.right() - END_GRAB / 2.0 - 1.0,
        ] {
            painter.circle_filled(egui::pos2(x, thumb.center().y), 1.5, dot);
        }
        let hovered = response
            .hover_pos()
            .filter(|pos| thumb.contains(*pos))
            .is_some_and(|pos| pos.x < thumb.left() + END_GRAB || pos.x > thumb.right() - END_GRAB);
        if hovered || matches!(grab, Some(Grab::Start { .. } | Grab::End { .. })) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
    }

    /// Scrolls and zooms so the view runs from `start` to `end`.
    fn show_span(&mut self, start: Ticks, end: Ticks) {
        let span = (end - start).0.max(1) as f64;
        self.ticks_per_point = (span / f64::from(self.width.max(1.0))).clamp(1.0, 1.0e6);
        self.scroll = start;
    }
}
