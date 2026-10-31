//! Rulers along the program monitor's top and left edges, and guides over the frame:
//! dragged out of a ruler, moved by dragging, and taken away by dragging them back onto
//! a ruler or out of the monitor.

use egui::{Align2, Color32, CursorIcon, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use timeline::{Guide, GuideAxis, GuideEdit};

use crate::editor::Editor;

const RULER: f32 = 16.0;
const GUIDE: Color32 = Color32::from_rgb(70, 205, 235);
/// How near, in points, the pointer must be to a guide to drag it.
const GRAB_REACH: f32 = 3.0;
/// The least room, in points, between labelled ruler ticks.
const LABEL_SPACING: f32 = 60.0;

/// What the program monitor shows of the rulers and guides.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GuideView {
    pub(crate) rulers: bool,
    pub(crate) shown: bool,
    pub(crate) locked: bool,
}

impl Default for GuideView {
    fn default() -> Self {
        Self {
            rulers: false,
            shown: true,
            locked: false,
        }
    }
}

/// A guide being dragged: one already laid, by index, or a new one out of a ruler.
#[derive(Debug, Clone, Copy)]
struct Dragged {
    axis: GuideAxis,
    index: Option<usize>,
}

/// Where sequence pixels land over the program picture.
#[derive(Debug, Clone, Copy)]
struct Placing {
    picture: Rect,
    per_pixel: f32,
}

impl Placing {
    fn of(editor: &Editor, picture: Rect) -> Self {
        let width = editor.timeline.sequence().format.width.max(1);
        Self {
            picture,
            per_pixel: picture.width() / width as f32,
        }
    }

    fn screen(&self, axis: GuideAxis, at: f32) -> f32 {
        match axis {
            GuideAxis::Vertical => self.picture.left() + at * self.per_pixel,
            GuideAxis::Horizontal => self.picture.top() + at * self.per_pixel,
        }
    }

    fn pixels(&self, axis: GuideAxis, pointer: Pos2) -> f32 {
        let from = match axis {
            GuideAxis::Vertical => pointer.x - self.picture.left(),
            GuideAxis::Horizontal => pointer.y - self.picture.top(),
        };
        from / self.per_pixel.max(f32::EPSILON)
    }
}

/// The screen positions of the shown vertical and horizontal guides, for snapping to.
pub(crate) fn snap_lines(editor: &Editor, picture: Rect) -> (Vec<f32>, Vec<f32>) {
    if !editor.guide_view.shown {
        return (Vec::new(), Vec::new());
    }
    let placing = Placing::of(editor, picture);
    let guides = &editor.timeline.sequence().guides;
    let along = |axis: GuideAxis| {
        guides
            .iter()
            .filter(|guide| guide.axis == axis)
            .map(|guide| placing.screen(axis, guide.at as f32))
            .collect()
    };
    (along(GuideAxis::Vertical), along(GuideAxis::Horizontal))
}

/// Draws the rulers and guides over the program `picture`, within `ui`'s rect, and lets
/// guides be dragged; drawn after the picture's own handles so a guide takes the drag.
pub(crate) fn overlay(editor: &mut Editor, ui: &egui::Ui, picture: Rect) {
    let view = editor.guide_view;
    if !view.rulers && !(view.shown && !editor.timeline.sequence().guides.is_empty()) {
        return;
    }
    let area = ui.max_rect();
    let placing = Placing::of(editor, picture);
    let painter = ui.painter_at(area);
    let id = ui.make_persistent_id("guides");
    let mut dragged = ui.data(|data| data.get_temp::<Dragged>(id));
    let [top, left] = rulers(area);
    if view.rulers {
        for (rect, axis) in [(top, GuideAxis::Horizontal), (left, GuideAxis::Vertical)] {
            let response = ui.interact(rect, id.with(axis), Sense::drag());
            if response.drag_started() {
                dragged = Some(Dragged { axis, index: None });
            }
        }
    }
    let guides = editor.timeline.sequence().guides.clone();
    if view.shown {
        for (index, guide) in guides.iter().enumerate() {
            let at = placing.screen(guide.axis, guide.at as f32);
            if !view.locked {
                let reach = line_rect(area, guide.axis, at).expand(GRAB_REACH);
                let response = ui.interact(reach, id.with(index), Sense::drag());
                if response.hovered() {
                    ui.ctx().set_cursor_icon(cursor(guide.axis));
                }
                if response.drag_started() {
                    dragged = Some(Dragged {
                        axis: guide.axis,
                        index: Some(index),
                    });
                }
            }
            let moving = dragged.is_some_and(|drag| drag.index == Some(index));
            if !moving {
                paint_line(&painter, area, guide.axis, at);
            }
        }
    }
    if let Some(drag) = dragged {
        let pointer = ui.input(|input| input.pointer.latest_pos());
        if ui.input(|input| input.pointer.primary_down()) {
            ui.ctx().set_cursor_icon(cursor(drag.axis));
            if let Some(pointer) = pointer {
                show_dragged(&painter, area, &placing, drag.axis, pointer, editor);
            }
        } else {
            dragged = None;
            let dropped = pointer.filter(|&pointer| {
                area.contains(pointer)
                    && !(view.rulers && (top.contains(pointer) || left.contains(pointer)))
            });
            drop_guide(editor, &placing, drag, dropped);
        }
    }
    ui.data_mut(|data| match dragged {
        Some(drag) => {
            data.insert_temp(id, drag);
        }
        None => data.remove::<Dragged>(id),
    });
    if view.rulers {
        paint_ruler(&painter, top, &placing, GuideAxis::Vertical);
        paint_ruler(&painter, left, &placing, GuideAxis::Horizontal);
        painter.rect_filled(top.intersect(left), 0.0, ui.visuals().extreme_bg_color);
    }
}

/// Lays or moves the guide where it was let go, or takes it away if it wasn't over
/// the frame.
fn drop_guide(editor: &mut Editor, placing: &Placing, drag: Dragged, at: Option<Pos2>) {
    let format = editor.timeline.sequence().format;
    let limit = match drag.axis {
        GuideAxis::Vertical => format.width,
        GuideAxis::Horizontal => format.height,
    };
    let at = at.map(|pointer| {
        let pixels = placing.pixels(drag.axis, pointer).round();
        pixels.clamp(0.0, limit as f32) as i32
    });
    let edit = match (drag.index, at) {
        (Some(index), Some(to)) => GuideEdit::Move { index, to },
        (Some(index), None) => GuideEdit::Remove(index),
        (None, Some(at)) => {
            editor.guide_view.shown = true;
            GuideEdit::Add(Guide {
                axis: drag.axis,
                at,
            })
        }
        (None, None) => return,
    };
    editor.apply(edit);
}

fn show_dragged(
    painter: &egui::Painter,
    area: Rect,
    placing: &Placing,
    axis: GuideAxis,
    pointer: Pos2,
    editor: &Editor,
) {
    let at = match axis {
        GuideAxis::Vertical => pointer.x,
        GuideAxis::Horizontal => pointer.y,
    };
    paint_line(painter, area, axis, at);
    let format = editor.timeline.sequence().format;
    let pixels = placing.pixels(axis, pointer).round();
    let label = format!(
        "{pixels:.0} px of {}",
        match axis {
            GuideAxis::Vertical => format.width,
            GuideAxis::Horizontal => format.height,
        }
    );
    painter.text(
        pointer + Vec2::new(10.0, 10.0),
        Align2::LEFT_TOP,
        label,
        FontId::proportional(12.0),
        GUIDE,
    );
}

/// The top and the left ruler.
fn rulers(area: Rect) -> [Rect; 2] {
    [
        Rect::from_min_max(area.min, egui::pos2(area.right(), area.top() + RULER)),
        Rect::from_min_max(area.min, egui::pos2(area.left() + RULER, area.bottom())),
    ]
}

fn line_rect(area: Rect, axis: GuideAxis, at: f32) -> Rect {
    match axis {
        GuideAxis::Vertical => Rect::from_x_y_ranges(at..=at, area.y_range()),
        GuideAxis::Horizontal => Rect::from_x_y_ranges(area.x_range(), at..=at),
    }
}

fn paint_line(painter: &egui::Painter, area: Rect, axis: GuideAxis, at: f32) {
    let stroke = Stroke::new(1.0, GUIDE);
    match axis {
        GuideAxis::Vertical => painter.vline(at, area.y_range(), stroke),
        GuideAxis::Horizontal => painter.hline(area.x_range(), at, stroke),
    };
}

fn cursor(axis: GuideAxis) -> CursorIcon {
    match axis {
        GuideAxis::Vertical => CursorIcon::ResizeHorizontal,
        GuideAxis::Horizontal => CursorIcon::ResizeVertical,
    }
}

/// A ruler counting sequence pixels from the frame's edge; `axis` is the guides it
/// measures, so the top ruler measures vertical ones.
fn paint_ruler(painter: &egui::Painter, rect: Rect, placing: &Placing, axis: GuideAxis) {
    painter.rect_filled(rect, 0.0, Color32::from_black_alpha(200));
    let step = tick_step(placing.per_pixel);
    let ends = match axis {
        GuideAxis::Vertical => [rect.left(), rect.right()],
        GuideAxis::Horizontal => [rect.top(), rect.bottom()],
    };
    let pixels = ends.map(|end| match axis {
        GuideAxis::Vertical => placing.pixels(axis, egui::pos2(end, 0.0)),
        GuideAxis::Horizontal => placing.pixels(axis, egui::pos2(0.0, end)),
    });
    let minor = step / 5.0;
    let first = (pixels[0] / minor).floor() as i64;
    let last = (pixels[1] / minor).ceil() as i64;
    let color = Color32::from_gray(170);
    for tick in first..=last {
        let value = tick as f32 * minor;
        let at = placing.screen(axis, value);
        let major = tick % 5 == 0;
        let length = if major { RULER } else { RULER * 0.3 };
        let (from, to) = match axis {
            GuideAxis::Vertical => (
                egui::pos2(at, rect.bottom() - length),
                egui::pos2(at, rect.bottom()),
            ),
            GuideAxis::Horizontal => (
                egui::pos2(rect.right() - length, at),
                egui::pos2(rect.right(), at),
            ),
        };
        painter.line_segment([from, to], Stroke::new(1.0, color));
        if major {
            painter.text(
                from + Vec2::new(3.0, 1.0),
                Align2::LEFT_TOP,
                format!("{value:.0}"),
                FontId::monospace(9.0),
                color,
            );
        }
    }
}

/// Sequence pixels between labelled ticks: 1, 2 or 5 times a power of ten, as few as
/// leave room for their labels.
fn tick_step(per_pixel: f32) -> f32 {
    let least = LABEL_SPACING / per_pixel.max(f32::EPSILON);
    let power = 10f32.powf(least.log10().floor());
    [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .map(|factor| factor * power)
        .find(|&step| step >= least)
        .unwrap_or(power * 10.0)
}

/// The Program monitor's Rulers and Guides menu.
pub(crate) fn menu(ui: &mut egui::Ui, editor: &mut Editor) {
    ui.menu_button("Rulers and Guides", |ui| {
        let view = &mut editor.guide_view;
        ui.checkbox(&mut view.rulers, "Show Rulers")
            .on_hover_text("Drag out of a ruler to add a guide");
        ui.checkbox(&mut view.shown, "Show Guides");
        ui.checkbox(&mut view.locked, "Lock Guides");
        ui.separator();
        let format = editor.timeline.sequence().format;
        let centered = [
            ("Add Vertical Guide", GuideAxis::Vertical, format.width / 2),
            (
                "Add Horizontal Guide",
                GuideAxis::Horizontal,
                format.height / 2,
            ),
        ];
        for (label, axis, middle) in centered {
            if ui
                .button(label)
                .on_hover_text("Across the middle of the frame")
                .clicked()
            {
                editor.guide_view.shown = true;
                let at = i32::try_from(middle).unwrap_or(i32::MAX);
                editor.apply(GuideEdit::Add(Guide { axis, at }));
            }
        }
        let none = editor.timeline.sequence().guides.is_empty();
        if ui
            .add_enabled(!none, egui::Button::new("Clear Guides"))
            .clicked()
        {
            editor.apply(GuideEdit::Clear);
        }
    });
}
