use media::TrackDirection;
use timeline::Interpolation;

use super::look::ANIMATED;
use crate::mask_tracking::TrackSpan;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Icon {
    /// Points down when what it discloses is open.
    Twirl(bool),
    Stopwatch,
    Previous,
    Key(Interpolation),
    Next,
    /// A triangle the way it tracks, with a bar after it for a single frame.
    Track(TrackDirection, TrackSpan),
}

/// A small painted button; the default fonts have no glyphs for these symbols.
pub(super) fn icon_button(
    ui: &mut egui::Ui,
    icon: Icon,
    filled: bool,
    enabled: bool,
) -> egui::Response {
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(16.0, 18.0), sense);
    let visuals = ui.style().interact(&response);
    let mut color = if filled {
        ANIMATED
    } else {
        visuals.fg_stroke.color
    };
    if !enabled {
        color = color.gamma_multiply(0.35);
    }
    let fill = if filled {
        color
    } else {
        egui::Color32::TRANSPARENT
    };
    let stroke = egui::Stroke::new(1.2, color);
    let c = rect.center();
    let at = |points: &[(f32, f32)]| points.iter().map(|&(x, y)| c + egui::vec2(x, y)).collect();
    let painter = ui.painter();
    match icon {
        Icon::Twirl(open) => {
            let points: &[(f32, f32)] = if open {
                &[(-4.0, -2.0), (4.0, -2.0), (0.0, 3.0)]
            } else {
                &[(-2.0, -4.0), (-2.0, 4.0), (3.0, 0.0)]
            };
            painter.add(egui::Shape::convex_polygon(at(points), color, stroke));
        }
        Icon::Stopwatch => {
            painter.circle(c + egui::vec2(0.0, 1.0), 5.0, fill, stroke);
            painter.line_segment(
                [c + egui::vec2(0.0, -7.0), c + egui::vec2(0.0, -4.0)],
                stroke,
            );
        }
        Icon::Previous => {
            painter.add(egui::Shape::convex_polygon(
                at(&[(3.0, -4.0), (3.0, 4.0), (-3.0, 0.0)]),
                color,
                stroke,
            ));
        }
        Icon::Next => {
            painter.add(egui::Shape::convex_polygon(
                at(&[(-3.0, -4.0), (-3.0, 4.0), (3.0, 0.0)]),
                color,
                stroke,
            ));
        }
        Icon::Key(interpolation) => paint_key(painter, c, interpolation, fill, stroke),
        Icon::Track(direction, span) => {
            let way = match direction {
                TrackDirection::Forward => 1.0,
                TrackDirection::Backward => -1.0,
            };
            let back = if span == TrackSpan::OneFrame {
                -1.5
            } else {
                0.0
            };
            let triangle =
                [(-3.0, -4.0), (-3.0, 4.0), (3.0, 0.0)].map(|(x, y)| (way * (x + back), y));
            painter.add(egui::Shape::convex_polygon(at(&triangle), color, stroke));
            if span == TrackSpan::OneFrame {
                let x = way * 4.5;
                painter.line_segment([c + egui::vec2(x, -4.0), c + egui::vec2(x, 4.0)], stroke);
            }
        }
    }
    response
}

/// A diamond for linear keyframes, a square for holds, an hourglass for Bezier keyframes
/// and a circle for eases.
pub(super) fn paint_key(
    painter: &egui::Painter,
    center: egui::Pos2,
    interpolation: Interpolation,
    fill: egui::Color32,
    stroke: egui::Stroke,
) {
    let at = |points: &[(f32, f32)]| {
        points
            .iter()
            .map(|&(x, y)| center + egui::vec2(x, y))
            .collect()
    };
    match interpolation {
        Interpolation::Linear => {
            painter.add(egui::Shape::convex_polygon(
                at(&[(0.0, -5.0), (5.0, 0.0), (0.0, 5.0), (-5.0, 0.0)]),
                fill,
                stroke,
            ));
        }
        Interpolation::Hold => {
            painter.add(egui::Shape::convex_polygon(
                at(&[(-4.0, -4.0), (4.0, -4.0), (4.0, 4.0), (-4.0, 4.0)]),
                fill,
                stroke,
            ));
        }
        Interpolation::Bezier | Interpolation::ContinuousBezier => {
            for tip in [-5.0, 5.0] {
                painter.add(egui::Shape::convex_polygon(
                    at(&[(-4.0, tip), (4.0, tip), (0.0, 0.0)]),
                    fill,
                    stroke,
                ));
            }
        }
        _ => {
            painter.circle(center, 4.5, fill, stroke);
        }
    }
}
