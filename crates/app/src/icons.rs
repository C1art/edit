//! Small line icons painted as shapes, so they stay crisp at any scale and don't
//! depend on which symbols the fonts happen to have.

use std::f32::consts::PI;

use egui::{pos2, vec2, Color32, Painter, Pos2, Rect, Response, Sense, Shape, Stroke};

use crate::theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Icon {
    Eye,
    Speaker,
    Lock,
    Link,
    Solo,
    Keyframes,
    Play,
    Pause,
    StepBack,
    StepForward,
    Loop,
    Back,
    Up,
    Folder,
    File,
}

const BUTTON: egui::Vec2 = vec2(22.0, 20.0);
const GLYPH: f32 = 12.0;

/// How a toggle shows that it is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Look {
    /// Filled in the accent.
    Filled,
    /// Filled in the accent and struck through, as hidden and muted tracks are.
    Crossed,
    /// Only brightened, for switches that are usually on.
    Quiet,
}

/// A flat button showing `icon`, `size` across.
pub(crate) fn button(ui: &mut egui::Ui, icon: Icon, size: egui::Vec2) -> Response {
    sized_toggle(ui, false, icon, Look::Filled, size)
}

/// A square toggle showing `icon`.
pub(crate) fn toggle(ui: &mut egui::Ui, on: bool, icon: Icon, look: Look) -> Response {
    sized_toggle(ui, on, icon, look, BUTTON)
}

pub(crate) fn sized_toggle(
    ui: &mut egui::Ui,
    on: bool,
    icon: Icon,
    look: Look,
    size: egui::Vec2,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let fill = match (on && look != Look::Quiet, response.hovered()) {
        (true, _) => ui.visuals().selection.bg_fill,
        (false, true) => ui.visuals().widgets.hovered.bg_fill,
        (false, false) => Color32::TRANSPARENT,
    };
    let painter = ui.painter();
    painter.rect_filled(rect, 4.0, fill);
    let color = if on || response.hovered() {
        Color32::WHITE
    } else {
        theme::WEAK_TEXT
    };
    let glyph = Rect::from_center_size(rect.center(), vec2(GLYPH, GLYPH));
    paint(painter, glyph, icon, color);
    if on && look == Look::Crossed {
        let slash = glyph.expand(1.0);
        painter.line_segment(
            [slash.left_bottom(), slash.right_top()],
            Stroke::new(1.5, color),
        );
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub(crate) fn paint(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let stroke = Stroke::new(1.3, color);
    let at = |x: f32, y: f32| {
        pos2(
            rect.left() + x * rect.width(),
            rect.top() + y * rect.height(),
        )
    };
    match icon {
        Icon::Eye => {
            let lid = |sign: f32| -> Vec<Pos2> {
                (0..=12)
                    .map(|step| {
                        let t = step as f32 / 12.0;
                        at(t, 0.5 + sign * 0.36 * (PI * t).sin())
                    })
                    .collect()
            };
            painter.add(Shape::line(lid(-1.0), stroke));
            painter.add(Shape::line(lid(1.0), stroke));
            painter.circle_filled(rect.center(), rect.width() * 0.16, color);
        }
        Icon::Speaker => {
            let cone = vec![
                at(0.05, 0.35),
                at(0.3, 0.35),
                at(0.58, 0.1),
                at(0.58, 0.9),
                at(0.3, 0.65),
                at(0.05, 0.65),
            ];
            painter.add(Shape::convex_polygon(cone, color, Stroke::NONE));
            let wave: Vec<Pos2> = (0..=8)
                .map(|step| {
                    let angle = -PI / 3.0 + step as f32 / 8.0 * (2.0 * PI / 3.0);
                    let center = at(0.55, 0.5);
                    center + rect.width() * 0.36 * vec2(angle.cos(), angle.sin())
                })
                .collect();
            painter.add(Shape::line(wave, stroke));
        }
        Icon::Lock => {
            let body = Rect::from_min_max(at(0.12, 0.45), at(0.88, 1.0));
            painter.rect_filled(body, 2.0, color);
            let shackle: Vec<Pos2> = (0..=10)
                .map(|step| {
                    let angle = PI + step as f32 / 10.0 * PI;
                    at(0.5, 0.42) + rect.width() * 0.26 * vec2(angle.cos(), angle.sin())
                })
                .collect();
            let mut shackle = shackle;
            shackle.insert(0, at(0.24, 0.47));
            shackle.push(at(0.76, 0.47));
            painter.add(Shape::line(shackle, stroke));
        }
        Icon::Link => {
            let size = vec2(rect.width() * 0.55, rect.height() * 0.38);
            for center in [at(0.32, 0.5), at(0.68, 0.5)] {
                let link = Rect::from_center_size(center, size);
                painter.rect_stroke(link, size.y / 2.0, stroke, egui::StrokeKind::Middle);
            }
        }
        Icon::Solo => {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "S",
                egui::FontId::proportional(rect.height() * 1.05),
                color,
            );
        }
        Icon::Play => {
            let triangle = vec![at(0.18, 0.05), at(0.92, 0.5), at(0.18, 0.95)];
            painter.add(Shape::convex_polygon(triangle, color, Stroke::NONE));
        }
        Icon::Pause => {
            for left in [0.18, 0.6] {
                let bar = Rect::from_min_max(at(left, 0.08), at(left + 0.22, 0.92));
                painter.rect_filled(bar, 1.0, color);
            }
        }
        Icon::StepBack | Icon::StepForward => {
            let flip = |x: f32| if icon == Icon::StepBack { 1.0 - x } else { x };
            let triangle = vec![
                at(flip(0.12), 0.12),
                at(flip(0.7), 0.5),
                at(flip(0.12), 0.88),
            ];
            painter.add(Shape::convex_polygon(triangle, color, Stroke::NONE));
            let (near, far) = (flip(0.74), flip(0.9));
            let bar = Rect::from_min_max(at(near.min(far), 0.12), at(near.max(far), 0.88));
            painter.rect_filled(bar, 1.0, color);
        }
        Icon::Loop => {
            let center = rect.center();
            let radius = rect.width() * 0.4;
            let arc: Vec<Pos2> = (0..=16)
                .map(|step| {
                    let angle = -PI * 0.35 + step as f32 / 16.0 * PI * 1.7;
                    center + radius * vec2(angle.cos(), angle.sin())
                })
                .collect();
            let tip = arc[0];
            painter.add(Shape::line(arc, stroke));
            let head = rect.width() * 0.22;
            let arrow = vec![
                tip + vec2(-head, -head * 0.2),
                tip + vec2(head * 0.3, -head),
                tip + vec2(head * 0.4, head * 0.25),
            ];
            painter.add(Shape::convex_polygon(arrow, color, Stroke::NONE));
        }
        Icon::Keyframes => {
            let line = vec![at(0.0, 0.8), at(0.35, 0.3), at(0.65, 0.6), at(1.0, 0.2)];
            painter.add(Shape::line(line.clone(), stroke));
            for point in &line[1..3] {
                let radius = rect.width() * 0.14;
                let diamond = vec![
                    *point + vec2(0.0, -radius),
                    *point + vec2(radius, 0.0),
                    *point + vec2(0.0, radius),
                    *point + vec2(-radius, 0.0),
                ];
                painter.add(Shape::convex_polygon(diamond, color, Stroke::NONE));
            }
        }
        Icon::Back | Icon::Up => {
            // Drawn pointing left, then turned for up.
            let turn = |x: f32, y: f32| if icon == Icon::Up { at(y, x) } else { at(x, y) };
            painter.line_segment([turn(0.92, 0.5), turn(0.12, 0.5)], stroke);
            let head = vec![turn(0.48, 0.14), turn(0.12, 0.5), turn(0.48, 0.86)];
            painter.add(Shape::line(head, stroke));
        }
        Icon::Folder => {
            let tab = Rect::from_min_max(at(0.04, 0.1), at(0.46, 0.34));
            painter.rect_filled(tab, 1.5, color);
            let body = Rect::from_min_max(at(0.04, 0.22), at(0.96, 0.9));
            painter.rect_filled(body, 1.5, color);
        }
        Icon::File => {
            let page = vec![
                at(0.16, 0.04),
                at(0.6, 0.04),
                at(0.86, 0.3),
                at(0.86, 0.96),
                at(0.16, 0.96),
            ];
            painter.add(Shape::convex_polygon(page, color, Stroke::NONE));
        }
    }
}
