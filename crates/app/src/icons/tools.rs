use egui::{vec2, Color32, Painter, Pos2, Rect, Shape, Stroke};
use timeline::ShapeKind;

use super::Icon;
use crate::timeline_view::{Direction, Tool};

/// Draws `tool`'s icon in `rect`, in the manner of other editors' tool panels.
pub(super) fn paint(painter: &Painter, rect: Rect, tool: Tool, color: Color32) {
    let stroke = Stroke::new(1.3, color);
    let at = |x: f32, y: f32| rect.lerp_inside(vec2(x, y));
    let line = |points: &[(f32, f32)]| {
        let points: Vec<Pos2> = points.iter().map(|&(x, y)| at(x, y)).collect();
        painter.add(Shape::line(points, stroke));
    };
    let filled = |points: &[(f32, f32)]| {
        let points: Vec<Pos2> = points.iter().map(|&(x, y)| at(x, y)).collect();
        painter.add(Shape::convex_polygon(points, color, Stroke::NONE));
    };
    // A double-headed arrow along the middle, from `from` to `to`.
    let arrows = |from: f32, to: f32| {
        line(&[(from, 0.5), (to, 0.5)]);
        line(&[(from + 0.16, 0.34), (from, 0.5), (from + 0.16, 0.66)]);
        line(&[(to - 0.16, 0.34), (to, 0.5), (to - 0.16, 0.66)]);
    };
    match tool {
        Tool::Selection => {
            filled(&[(0.2, 0.0), (0.2, 0.8), (0.78, 0.56)]);
            painter.line_segment([at(0.4, 0.55), at(0.58, 0.98)], Stroke::new(2.2, color));
        }
        Tool::TrackSelect(direction) => {
            let x = |x: f32| match direction {
                Direction::Forward => x,
                Direction::Backward => 1.0 - x,
            };
            line(&[(x(0.06), 0.05), (x(0.06), 0.95)]);
            for start in [0.24, 0.6] {
                filled(&[(x(start), 0.2), (x(start + 0.36), 0.5), (x(start), 0.8)]);
            }
        }
        Tool::Ripple => {
            line(&[(0.7, 0.05), (0.55, 0.05), (0.55, 0.95), (0.7, 0.95)]);
            arrows(0.0, 0.42);
            painter.rect_filled(Rect::from_min_max(at(0.66, 0.3), at(1.0, 0.7)), 1.0, color);
        }
        Tool::Roll => {
            line(&[(0.3, 0.05), (0.45, 0.05), (0.45, 0.95), (0.3, 0.95)]);
            line(&[(0.7, 0.05), (0.55, 0.05), (0.55, 0.95), (0.7, 0.95)]);
            line(&[(0.0, 0.5), (0.36, 0.5)]);
            line(&[(0.64, 0.5), (1.0, 0.5)]);
        }
        Tool::RateStretch => {
            line(&[(0.02, 0.1), (0.02, 0.9)]);
            line(&[(0.98, 0.1), (0.98, 0.9)]);
            let spring: Vec<(f32, f32)> = (0..=8)
                .map(|step| {
                    let y = if step % 2 == 0 {
                        0.5
                    } else if step % 4 == 1 {
                        0.25
                    } else {
                        0.75
                    };
                    (0.1 + step as f32 * 0.1, y)
                })
                .collect();
            line(&spring);
        }
        Tool::Razor => {
            filled(&[(0.05, 0.78), (0.62, 0.05), (0.95, 0.3), (0.38, 1.0)]);
            painter.line_segment(
                [at(0.36, 0.56), at(0.62, 0.34)],
                Stroke::new(1.4, Color32::BLACK),
            );
        }
        Tool::Slip => {
            line(&[(0.04, 0.1), (0.04, 0.9)]);
            line(&[(0.96, 0.1), (0.96, 0.9)]);
            arrows(0.18, 0.82);
        }
        Tool::Slide => {
            painter.rect_filled(Rect::from_min_max(at(0.34, 0.2), at(0.66, 0.8)), 1.0, color);
            line(&[(0.0, 0.5), (0.24, 0.5)]);
            line(&[(0.12, 0.36), (0.0, 0.5), (0.12, 0.64)]);
            line(&[(0.76, 0.5), (1.0, 0.5)]);
            line(&[(0.88, 0.36), (1.0, 0.5), (0.88, 0.64)]);
        }
        Tool::Hand => {
            let finger = Stroke::new(2.0, color);
            for (x, top) in [(0.36, 0.12), (0.52, 0.04), (0.68, 0.1), (0.84, 0.24)] {
                painter.line_segment([at(x, top), at(x, 0.55)], finger);
            }
            painter.line_segment([at(0.1, 0.48), at(0.3, 0.72)], finger);
            painter.rect_filled(
                Rect::from_min_max(at(0.28, 0.48), at(0.9, 0.98)),
                3.0,
                color,
            );
        }
        Tool::Zoom => {
            super::paint(painter, rect, Icon::Search, color);
            let lens = at(0.4, 0.4);
            let arm = rect.width() * 0.14;
            painter.line_segment([lens - vec2(arm, 0.0), lens + vec2(arm, 0.0)], stroke);
            painter.line_segment([lens - vec2(0.0, arm), lens + vec2(0.0, arm)], stroke);
        }
        Tool::Type => {
            let bold = Stroke::new(2.0, color);
            painter.line_segment([at(0.1, 0.1), at(0.9, 0.1)], bold);
            painter.line_segment([at(0.5, 0.1), at(0.5, 0.95)], bold);
        }
        Tool::Shape(ShapeKind::Rectangle) => super::paint(painter, rect, Icon::Rectangle, color),
        Tool::Shape(ShapeKind::Ellipse) => super::paint(painter, rect, Icon::Ellipse, color),
    }
}
