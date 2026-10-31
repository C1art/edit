//! Crop edges on the program picture while the Crop section is open, as Premiere shows
//! them with Crop selected: an edge or corner drags that side in, and dragging inside
//! moves the cropped window across the picture.

use egui::{Color32, CursorIcon, Pos2, Rect, Sense, Shape, Stroke, Vec2};
use timeline::{Crop, SetClipLook, Ticks};

use super::motion_overlay::moved_clip;
use super::picture_frame::PictureFrame;
use super::{adjust_look, keyframe_time, shown_look};
use crate::editor::Editor;
use crate::mixer::Change;

const OUTLINE: Color32 = Color32::from_rgb(220, 220, 220);
const HANDLE_SIZE: f32 = 7.0;
/// How near, in points, the pointer must be to grab an edge or corner.
const REACH: f32 = 6.0;
/// The least of the picture, in percent, that a crop leaves showing.
const SHOWING: f32 = 1.0;

/// Which crop sides a drag moves, left, top, right and bottom; all of them moves the
/// window.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Grip([bool; 4]);

impl Grip {
    const WINDOW: Self = Self([true; 4]);
}

#[derive(Debug, Clone, Copy)]
struct Held {
    grip: Grip,
    from: Pos2,
    /// The crop at the playhead when the drag started, percent per side.
    start: [f32; 4],
}

pub(super) fn crop_overlay(editor: &mut Editor, ui: &egui::Ui, area: Rect) {
    let Some(clip) = moved_clip(editor) else {
        return;
    };
    let time = keyframe_time(editor, &clip);
    let mut look = shown_look(editor, &clip);
    let Some(frame) = PictureFrame::new(editor.timeline.sequence(), &clip, &look, time, area)
    else {
        return;
    };
    let sides = sides_at(&look.crop, time);
    let corners = cropped_corners(&frame, sides);
    let reach = Rect::from_points(&corners).expand(REACH * 2.0);
    let id = ui.make_persistent_id(("crop_overlay", clip.id));
    let response = ui.interact(reach, id, Sense::drag());
    let mut held = ui.data(|data| data.get_temp::<Held>(id));
    let down = ui.input(|input| input.pointer.primary_down());
    let mut change = None;
    if held.is_none() && response.drag_started() {
        held = ui
            .input(|input| input.pointer.press_origin())
            .and_then(|from| {
                Some(Held {
                    grip: grip_at(&frame, &corners, sides, from)?,
                    from,
                    start: sides,
                })
            });
    }
    if let Some(drag) = held {
        if !down {
            held = None;
            if editor.effects.adjusting.is_some() {
                change = Some(Change::Commit);
            }
        } else if let Some(pointer) = ui.input(|input| input.pointer.latest_pos()) {
            let cropped = dragged(&frame, drag, pointer);
            let crop = &mut look.crop;
            for (side, percent) in [
                &mut crop.left,
                &mut crop.top,
                &mut crop.right,
                &mut crop.bottom,
            ]
            .into_iter()
            .zip(cropped)
            {
                side.set_at(time, (percent * 10.0).round() / 10.0);
            }
            change = Some(Change::Live);
        }
    }
    let hovered = held
        .map(|drag| drag.grip)
        .or_else(|| grip_at(&frame, &corners, sides, response.hover_pos()?));
    if let Some(Grip([left, top, right, bottom])) = hovered {
        ui.ctx()
            .set_cursor_icon(match (left || right, top || bottom) {
                (true, true) if left && right => CursorIcon::Move,
                (true, true) if left == top => CursorIcon::ResizeNwSe,
                (true, true) => CursorIcon::ResizeNeSw,
                (true, false) => CursorIcon::ResizeHorizontal,
                _ => CursorIcon::ResizeVertical,
            });
    }
    ui.data_mut(|data| match held {
        Some(held) => {
            data.insert_temp(id, held);
        }
        None => data.remove::<Held>(id),
    });
    paint(
        ui,
        &frame,
        &cropped_corners(&frame, sides_at(&look.crop, time)),
    );
    if change.is_some() {
        let edit = SetClipLook {
            clip: clip.id,
            look,
        };
        adjust_look(editor, edit, change);
    }
}

fn sides_at(crop: &Crop, time: Ticks) -> [f32; 4] {
    [&crop.left, &crop.top, &crop.right, &crop.bottom].map(|side| side.value_at(time))
}

/// The cropped picture's bounds in picture pixels from its center.
fn cropped_bounds(frame: &PictureFrame, [left, top, right, bottom]: [f32; 4]) -> Rect {
    let size = frame.fitted;
    let half = size / 2.0;
    Rect::from_min_max(
        Pos2::new(
            -half.x + left / 100.0 * size.x,
            -half.y + top / 100.0 * size.y,
        ),
        Pos2::new(
            half.x - right / 100.0 * size.x,
            half.y - bottom / 100.0 * size.y,
        ),
    )
}

/// Top left, top right, bottom right and bottom left, on screen.
fn cropped_corners(frame: &PictureFrame, sides: [f32; 4]) -> [Pos2; 4] {
    let bounds = cropped_bounds(frame, sides);
    [
        bounds.left_top(),
        bounds.right_top(),
        bounds.right_bottom(),
        bounds.left_bottom(),
    ]
    .map(|corner| frame.to_screen(corner.to_vec2()))
}

/// Corners first, then edges, then the window inside.
fn grip_at(frame: &PictureFrame, corners: &[Pos2; 4], sides: [f32; 4], at: Pos2) -> Option<Grip> {
    const CORNERS: [[bool; 4]; 4] = [
        [true, true, false, false],
        [false, true, true, false],
        [false, false, true, true],
        [true, false, false, true],
    ];
    if let Some(index) = corners
        .iter()
        .position(|corner| corner.distance(at) <= REACH)
    {
        return Some(Grip(CORNERS[index]));
    }
    // Edges run top, right, bottom, left between the corners.
    const EDGES: [[bool; 4]; 4] = [
        [false, true, false, false],
        [false, false, true, false],
        [false, false, false, true],
        [true, false, false, false],
    ];
    let near_edge = (0..4).position(|index| {
        let (from, to) = (corners[index], corners[(index + 1) % 4]);
        distance_to_segment(at, from, to) <= REACH
    });
    if let Some(index) = near_edge {
        return Some(Grip(EDGES[index]));
    }
    cropped_bounds(frame, sides)
        .contains(frame.to_picture(at).to_pos2())
        .then_some(Grip::WINDOW)
}

fn distance_to_segment(point: Pos2, from: Pos2, to: Pos2) -> f32 {
    let along = to - from;
    let t = ((point - from).dot(along) / along.length_sq().max(f32::EPSILON)).clamp(0.0, 1.0);
    point.distance(from + along * t)
}

/// The crop, percent per side, with `drag` taken to `pointer`; each side stops short of
/// the one across from it.
fn dragged(frame: &PictureFrame, drag: Held, pointer: Pos2) -> [f32; 4] {
    let size = frame.fitted.max(Vec2::splat(f32::EPSILON));
    let [left, top, right, bottom] = drag.start;
    if drag.grip == Grip::WINDOW {
        let moved = (frame.to_picture(pointer) - frame.to_picture(drag.from)) / size * 100.0;
        let x = moved.x.clamp(-left, right);
        let y = moved.y.clamp(-top, bottom);
        return [left + x, top + y, right - x, bottom - y];
    }
    let at = frame.to_picture(pointer) / size * 100.0 + Vec2::splat(50.0);
    let reached = [at.x, at.y, 100.0 - at.x, 100.0 - at.y];
    let mut sides = drag.start;
    for side in 0..4 {
        if drag.grip.0[side] {
            let across = drag.start[(side + 2) % 4];
            sides[side] = reached[side].clamp(0.0, (100.0 - across - SHOWING).max(0.0));
        }
    }
    sides
}

fn paint(ui: &egui::Ui, frame: &PictureFrame, corners: &[Pos2; 4]) {
    let painter = ui.painter();
    let whole = [
        Vec2::new(-0.5, -0.5),
        Vec2::new(0.5, -0.5),
        Vec2::new(0.5, 0.5),
        Vec2::new(-0.5, 0.5),
    ]
    .map(|corner| frame.to_screen(corner * frame.fitted));
    painter.add(Shape::dashed_line(
        &[whole.as_slice(), &whole[..1]].concat(),
        Stroke::new(1.0, OUTLINE.gamma_multiply(0.5)),
        4.0,
        4.0,
    ));
    let outline = Stroke::new(1.0, Color32::BLACK);
    painter.add(Shape::closed_line(
        corners.to_vec(),
        Stroke::new(1.0, OUTLINE),
    ));
    let middles = (0..4).map(|index| corners[index].lerp(corners[(index + 1) % 4], 0.5));
    for handle in corners.iter().copied().chain(middles) {
        let square = Rect::from_center_size(handle, Vec2::splat(HANDLE_SIZE));
        painter.rect_filled(square, 0.0, OUTLINE);
        painter.rect_stroke(square, 0.0, outline, egui::StrokeKind::Outside);
    }
}
