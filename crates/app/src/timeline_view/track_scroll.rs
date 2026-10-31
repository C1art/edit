use egui::{Rect, Sense};
use timeline::{Sequence, TrackRef};

use super::{layout, TimelineView};
use crate::theme;

/// Room below the last track, so its bottom edge can still be dragged to resize it.
const BOTTOM_ROOM: f32 = 20.0;
const BAR_WIDTH: f32 = 8.0;
const MIN_THUMB: f32 = 24.0;

impl TimelineView {
    fn track_overflow(&self, sequence: &Sequence, tracks: Rect) -> f32 {
        (layout::total_height(sequence, &self.heights) - tracks.height() + BOTTOM_ROOM).max(0.0)
    }

    pub(super) fn scroll_tracks(&mut self, delta: f32, sequence: &Sequence, tracks: Rect) {
        // A sideways pan with the Hand tool doesn't count as scrolling the tracks.
        self.tracks_scrolled |= delta != 0.0;
        self.scroll_y += delta;
        self.clamp_track_scroll(sequence, tracks);
    }

    fn clamp_track_scroll(&mut self, sequence: &Sequence, tracks: Rect) {
        let overflow = self.track_overflow(sequence, tracks);
        self.scroll_y = self.scroll_y.clamp(0.0, overflow);
    }

    /// Keeps the scroll in range as tracks come and go. Until the tracks are scrolled
    /// it centres the line between video and audio, as other editors do, so a short
    /// panel shows V1 and A1 rather than only video tracks. The top is then moved to the
    /// nearest track edge, so no track shows only half its header.
    pub(super) fn place_tracks(&mut self, sequence: &Sequence, tracks: Rect) {
        if !self.tracks_scrolled {
            let rows = layout::rows(sequence, &self.heights, 0.0);
            let divide = rows
                .iter()
                .find(|row| row.track == TrackRef::audio(0))
                .map_or(0.0, |row| row.top - layout::DIVIDER / 2.0);
            let centred = divide - tracks.height() / 2.0;
            self.scroll_y = rows
                .iter()
                .map(|row| row.top)
                .filter(|top| *top <= divide)
                .min_by(|a, b| (a - centred).abs().total_cmp(&(b - centred).abs()))
                .unwrap_or(centred);
        }
        self.clamp_track_scroll(sequence, tracks);
    }

    /// A thin bar over the right edge of the tracks, there only when they don't all fit.
    pub(super) fn track_scrollbar(&mut self, ui: &egui::Ui, sequence: &Sequence, tracks: Rect) {
        let overflow = self.track_overflow(sequence, tracks);
        if overflow <= 0.0 {
            return;
        }
        let bar = Rect::from_x_y_ranges(
            tracks.right() - BAR_WIDTH..=tracks.right(),
            tracks.y_range(),
        );
        let thumb_height = (bar.height() * tracks.height() / (tracks.height() + overflow))
            .clamp(MIN_THUMB.min(bar.height()), bar.height());
        let travel = (bar.height() - thumb_height).max(1.0);
        let response = ui.interact(
            bar,
            ui.scope_id().with("track scrollbar"),
            Sense::click_and_drag(),
        );
        if response.dragged() {
            let delta = response.drag_delta().y * overflow / travel;
            self.scroll_tracks(delta, sequence, tracks);
        } else if let Some(pointer) = response
            .interact_pointer_pos()
            .filter(|_| response.clicked())
        {
            let thumb_middle = bar.top() + self.scroll_y / overflow * travel + thumb_height / 2.0;
            let page = tracks.height() * 0.9;
            let delta = if pointer.y < thumb_middle {
                -page
            } else {
                page
            };
            self.scroll_tracks(delta, sequence, tracks);
        }

        let thumb_top = bar.top() + self.scroll_y / overflow * travel;
        let thumb = Rect::from_x_y_ranges(bar.x_range(), thumb_top..=thumb_top + thumb_height)
            .shrink2(egui::vec2(2.0, 1.0));
        let painter = ui.painter_at(tracks);
        painter.rect_filled(bar, 0.0, ui.visuals().extreme_bg_color.gamma_multiply(0.6));
        painter.rect_filled(thumb, thumb.width() / 2.0, theme::thumb_fill(&response));
    }
}
