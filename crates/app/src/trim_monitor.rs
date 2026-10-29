use std::sync::Arc;

use eframe::egui_wgpu::RenderState;
use egui::{Align2, Color32, FontId, Rect, Vec2};
use playback::FrameProbe;
use timeline::{Ticks, Timecode};

use crate::monitor::Monitor;
use crate::timeline_view::TrimView;

/// Premiere's trim monitor: while an edit is dragged the program monitor shows the
/// frames either side of it, composed on a worker from the sequence the drag would leave.
pub(crate) struct TrimMonitor {
    probe: FrameProbe,
    sides: [Monitor; 2],
    /// The drag offset last asked for, so a pointer at rest asks once.
    asked: Option<Ticks>,
    shown: [Option<Ticks>; 2],
}

impl TrimMonitor {
    pub(crate) fn new(render_state: &RenderState) -> Option<Self> {
        Some(Self {
            probe: FrameProbe::new().ok()?,
            sides: [0, 1].map(|_| Monitor::new(render_state.clone())),
            asked: None,
            shown: [None; 2],
        })
    }

    /// Forgets the last drag, so the next one asks afresh and shows nothing stale.
    pub(crate) fn reset(&mut self) {
        self.asked = None;
        self.shown = [None; 2];
    }

    /// Draws both sides into `area`, asking for new frames when the drag has moved.
    pub(crate) fn ui(&mut self, ui: &egui::Ui, area: Rect, view: &TrimView<'_>) {
        if self.asked != Some(view.delta) {
            self.asked = Some(view.delta);
            self.probe
                .request(Arc::new(view.sequence.clone()), view.times.to_vec());
        }
        let format = view.sequence.format;
        let canvas = (format.width, format.height);
        if let Some(frames) = self.probe.latest() {
            for ((monitor, shown), frame) in self.sides.iter_mut().zip(&mut self.shown).zip(&frames)
            {
                if monitor.present(frame, canvas).is_ok() {
                    *shown = Some(frame.pts);
                }
            }
        }
        for monitor in &mut self.sides {
            // A frame that can't be shown leaves the last one, as when presenting.
            let _ = monitor.catch_up(ui.ctx());
        }
        ui.ctx().request_repaint();
        let painter = ui.painter_at(area);
        painter.rect_filled(area, 0.0, Color32::BLACK);
        let half = Vec2::new(area.width() / 2.0, area.height());
        let (rate, timebase) = (format.frame_rate, view.sequence.timebase);
        for (index, (monitor, shown)) in self.sides.iter().zip(self.shown).enumerate() {
            let cell = Rect::from_min_size(area.min + Vec2::new(half.x * index as f32, 0.0), half);
            if shown.is_some() {
                monitor.paint(&painter, cell.shrink(2.0));
            }
            let time = shown.unwrap_or(view.times[index]);
            painter.text(
                cell.center_bottom() - Vec2::new(0.0, 6.0),
                Align2::CENTER_BOTTOM,
                Timecode::new(time, timebase, rate).to_string(),
                FontId::monospace(12.0),
                Color32::WHITE,
            );
        }
        let frames = rate.frame_at(view.delta, timebase);
        painter.text(
            area.center_top() + Vec2::new(0.0, 6.0),
            Align2::CENTER_TOP,
            format!("{frames:+}"),
            FontId::monospace(13.0),
            Color32::from_rgb(255, 210, 80),
        );
    }
}
