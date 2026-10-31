use egui::Rect;
use timeline::{Sequence, Ticks};

use super::TimelineView;

/// Premiere's Timeline Playback Auto-Scrolling: how the view keeps up with the playhead
/// while it plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum AutoScroll {
    None,
    #[default]
    Page,
    Smooth,
}

impl AutoScroll {
    pub(crate) const ALL: [Self; 3] = [Self::None, Self::Page, Self::Smooth];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::None => "No Scroll",
            Self::Page => "Page Scroll",
            Self::Smooth => "Smooth Scroll",
        }
    }

    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Page => "page",
            Self::Smooth => "smooth",
        }
    }

    pub(crate) fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|scroll| scroll.key() == key)
    }
}

const MAX_TICKS_PER_POINT: f64 = 1.0e6;

impl TimelineView {
    /// Zooms by `factor` (above one zooms in) keeping time at screen x `anchor` fixed;
    /// without one, the playhead where it's in view, else the middle.
    pub fn zoom(&mut self, factor: f32, anchor: Option<f32>) {
        let scale = self.scale(0.0);
        let anchor = anchor.unwrap_or_else(|| {
            let playhead = scale.x(self.last_playhead);
            if (0.0..=self.width).contains(&playhead) {
                playhead
            } else {
                self.width / 2.0
            }
        });
        let fixed = scale.time(anchor);
        // A sequence fitted beyond the usual limit keeps its zoom rather than jumping in.
        let widest = MAX_TICKS_PER_POINT.max(self.ticks_per_point);
        self.ticks_per_point = (self.ticks_per_point / f64::from(factor)).clamp(1.0, widest);
        self.scroll = (fixed - self.scale(0.0).span(anchor)).max(Ticks::ZERO);
    }

    pub fn zoom_to_fit(&mut self, sequence: &Sequence) {
        let duration = sequence.duration().max(Ticks(
            i64::from(sequence.timebase.den()) / i64::from(sequence.timebase.num()),
        ));
        self.ticks_per_point = (duration.0 as f64 / f64::from(self.width.max(1.0)) * 1.05).max(1.0);
        self.scroll = Ticks::ZERO;
        self.fitted = Some((self.width, self.ticks_per_point));
    }

    /// Fits the view to a sequence that just got its first clips, or again when the
    /// panel changes size while it still shows the whole sequence, as a switch of
    /// workspace does. Brings the playhead into view when it plays or jumps out of it.
    pub(super) fn keep_in_view(
        &mut self,
        ui: &egui::Ui,
        sequence: &Sequence,
        playhead: Ticks,
        playing: bool,
    ) {
        let filled = sequence.duration() > Ticks::ZERO;
        let resized_while_fitted = self.fitted.is_some_and(|(width, ticks_per_point)| {
            width != self.width
                && ticks_per_point == self.ticks_per_point
                && self.scroll == Ticks::ZERO
        });
        if (filled && !self.filled) || resized_while_fitted {
            self.zoom_to_fit(sequence);
        }
        self.filled = filled;
        // A jump by keyboard or an edit is followed too; a drag scrolls at the edges instead.
        let jumped = playhead != self.last_playhead && !ui.input(|input| input.pointer.any_down());
        self.last_playhead = playhead;
        if playing {
            match self.auto_scroll {
                AutoScroll::None => {}
                AutoScroll::Page => self.follow(playhead, 0.1),
                AutoScroll::Smooth => self.center_past_middle(playhead),
            }
        } else if jumped {
            self.follow(playhead, 0.5);
        }
    }

    pub(super) fn handle_scroll(
        &mut self,
        ui: &egui::Ui,
        area: Rect,
        tracks: Rect,
        sequence: &Sequence,
    ) {
        if !ui.rect_contains_pointer(area) {
            return;
        }
        let (mut scroll, mut zoom, pointer, alt) = ui.input(|input| {
            (
                input.smooth_scroll_delta,
                input.zoom_delta(),
                input.pointer.hover_pos(),
                input.modifiers.alt,
            )
        });
        if alt {
            // Alt-scrolling zooms, as in Premiere, alongside egui's Ctrl-scrolling.
            zoom *= (scroll.y / 200.0).exp();
            scroll.y = 0.0;
        }
        if zoom != 1.0 {
            self.zoom(zoom, pointer.map(|pos| pos.x - tracks.left()));
        }
        if scroll.x != 0.0 {
            let scale = self.scale(0.0);
            self.scroll = (self.scroll - scale.span(scroll.x)).max(Ticks::ZERO);
        }
        if scroll.y != 0.0 {
            self.scroll_tracks(-scroll.y, sequence, tracks);
        }
    }

    /// Holds the playhead in the middle once it gets there, the sequence moving under it.
    fn center_past_middle(&mut self, playhead: Ticks) {
        let half = self.width * 0.5;
        let scale = self.scale(0.0);
        let x = scale.x(playhead);
        if x > half || x < 0.0 {
            self.scroll = (playhead - scale.span(half)).max(Ticks::ZERO);
        }
    }

    /// Brings the playhead into view when it's out of it, `lead` of the way across.
    fn follow(&mut self, playhead: Ticks, lead: f32) {
        let width = self.width;
        let scale = self.scale(0.0);
        let x = scale.x(playhead);
        if x > width - 10.0 || x < 0.0 {
            self.scroll = (playhead - scale.span(width * lead)).max(Ticks::ZERO);
        }
    }
}
