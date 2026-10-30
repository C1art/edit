//! Snapping a picture dragged in the Program monitor to the frame's edges and center.

use egui::{Pos2, Rect, Stroke, Vec2};

use crate::theme;

/// How near, in points, a picture's edge or center must come to the frame's to snap.
const SNAP_REACH: f32 = 6.0;

/// The frame's lines a dragged picture lines up with, drawn while it does.
#[derive(Debug, Default)]
pub(super) struct Guides {
    x: Option<f32>,
    y: Option<f32>,
}

/// How far to nudge a picture with these `corners` on screen so that, along each axis,
/// whichever of its edges or center is nearest one of `frame`'s within reach meets it.
pub(super) fn snap(corners: [Pos2; 4], frame: Rect) -> (Vec2, Guides) {
    let bounds = Rect::from_points(&corners);
    let nearest = |picture: [f32; 3], lines: [f32; 3]| {
        picture
            .into_iter()
            .flat_map(|edge| lines.map(|line| (line - edge, line)))
            .filter(|(gap, _)| gap.abs() <= SNAP_REACH)
            .min_by(|a, b| a.0.abs().total_cmp(&b.0.abs()))
    };
    let x = nearest(
        [bounds.left(), bounds.center().x, bounds.right()],
        [frame.left(), frame.center().x, frame.right()],
    );
    let y = nearest(
        [bounds.top(), bounds.center().y, bounds.bottom()],
        [frame.top(), frame.center().y, frame.bottom()],
    );
    let gap = |snapped: Option<(f32, f32)>| snapped.map_or(0.0, |(gap, _)| gap);
    let guides = Guides {
        x: x.map(|(_, line)| line),
        y: y.map(|(_, line)| line),
    };
    (Vec2::new(gap(x), gap(y)), guides)
}

impl Guides {
    pub(super) fn paint(&self, ui: &egui::Ui, frame: Rect) {
        let painter = ui.painter_at(frame.expand(1.0));
        let stroke = Stroke::new(1.0, theme::ACCENT);
        if let Some(x) = self.x {
            painter.vline(x, frame.y_range(), stroke);
        }
        if let Some(y) = self.y {
            painter.hline(frame.x_range(), y, stroke);
        }
    }
}
