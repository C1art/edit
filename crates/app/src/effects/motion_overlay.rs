use egui::{Color32, CursorIcon, Pos2, Sense, Shape, Stroke, Vec2};
use timeline::{Clip, Motion, Param, SetClipLook, Ticks, TrackKind};

use super::motion_snap::{snap, Guides};
use super::picture_frame::PictureFrame;
use super::{adjust_look, keyframe_time, shown_look, SideTab};
use crate::editor::Editor;
use crate::mixer::Change;

const OUTLINE: Color32 = Color32::from_rgb(220, 220, 220);
const HANDLE_SIZE: f32 = 7.0;
/// How near, in points, the pointer must be to grab a corner to scale.
const SCALE_REACH: f32 = 8.0;
/// Past a corner by up to this many points, a drag turns the picture.
const ROTATE_REACH: f32 = 24.0;
/// Shift-dragged turns land on multiples of this many degrees.
const ROTATE_STEP: f32 = 15.0;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Grip {
    Move,
    Scale,
    Rotate,
    /// Moves the anchor point, and the position with it so the picture stays put.
    Anchor,
}

/// A drag in progress: what it holds, where it started and the motion it started from.
#[derive(Debug, Clone, Copy)]
struct Held {
    grip: Grip,
    from: Pos2,
    motion: Start,
}

/// The motion's values at the playhead when a drag started.
#[derive(Debug, Clone, Copy)]
struct Start {
    position: Vec2,
    /// Width and height, percent.
    scale: [f32; 2],
    rotation: f32,
    anchor: Vec2,
}

/// Outlines the selected clip's picture over the program picture in `area` while the
/// effect controls show, as Premiere does with Motion selected: dragging inside moves
/// it, snapping its edges and center to the frame's unless Ctrl is held, a corner scales
/// it, just past a corner turns it and the cross moves its anchor point. Masked clips
/// leave the picture to their mask handles.
pub fn motion_overlay(editor: &mut Editor, ui: &egui::Ui, area: egui::Rect) {
    if editor.side_tab != SideTab::EffectControls {
        return;
    }
    let Some(clip) = moved_clip(editor) else {
        return;
    };
    let time = keyframe_time(editor, &clip);
    let mut look = shown_look(editor, &clip);
    let sequence = editor.timeline.sequence();
    let Some(frame) = PictureFrame::new(sequence, &clip, &look, time, area) else {
        return;
    };
    let per_pixel = frame.shown.width() / sequence.format.width.max(1) as f32;
    let corners = corners(&frame);
    let id = ui.make_persistent_id(("motion_overlay", clip.id));
    let response = ui.interact(frame.shown.expand(ROTATE_REACH), id, Sense::drag());
    let mut held = ui.data(|data| data.get_temp::<Held>(id));
    let (down, shift, free) = ui.input(|input| {
        let modifiers = input.modifiers;
        (
            input.pointer.primary_down(),
            modifiers.shift,
            modifiers.command,
        )
    });
    let mut change = None;
    let mut guides = Guides::default();
    if held.is_none() && response.drag_started() {
        let origin = ui.input(|input| input.pointer.press_origin());
        held = origin
            .and_then(|origin| Some((origin, grip_at(&frame, &corners, origin)?)))
            .map(|(from, grip)| Held {
                grip,
                from,
                motion: Start::at(&look.motion, time),
            });
    }
    if let Some(drag) = held {
        if !down {
            held = None;
            if editor.effects.adjusting.is_some() {
                change = Some(Change::Commit);
            }
        } else if let Some(pointer) = ui.input(|input| input.pointer.latest_pos()) {
            let start = drag.motion;
            let center = frame.anchor;
            let motion = &mut look.motion;
            let dragged = (pointer - drag.from) / per_pixel.max(f32::EPSILON);
            match drag.grip {
                Grip::Move => {
                    let wanted = start.position + dragged;
                    let moved_by = (wanted - Start::at(motion, time).position) * per_pixel;
                    let nudge = if free {
                        Vec2::ZERO
                    } else {
                        let (nudge, lines) =
                            snap(corners.map(|corner| corner + moved_by), frame.shown);
                        guides = lines;
                        nudge
                    };
                    set_position(motion, time, wanted + nudge / per_pixel.max(f32::EPSILON));
                }
                Grip::Anchor => {
                    // The picture holds still, so its frame is the same as at the start.
                    let moved = frame.to_picture(pointer) - frame.to_picture(drag.from);
                    let anchor = start.anchor + moved * frame.flip;
                    let [x, y] = &mut motion.anchor;
                    x.set_at(time, anchor.x.round());
                    y.set_at(time, anchor.y.round());
                    set_position(motion, time, start.position + dragged);
                }
                Grip::Scale => {
                    let ratio = (pointer - center).length()
                        / (drag.from - center).length().max(f32::EPSILON);
                    let [width, height] = start
                        .scale
                        .map(|percent| (percent * ratio * 10.0).round() / 10.0);
                    motion.scale.set_at(time, height);
                    if !motion.uniform_scale {
                        motion.scale_width.set_at(time, width);
                    }
                }
                Grip::Rotate => {
                    let turn = (pointer - center).angle() - (drag.from - center).angle();
                    let mut degrees = start.rotation + turn.to_degrees();
                    if shift {
                        degrees = (degrees / ROTATE_STEP).round() * ROTATE_STEP;
                    }
                    motion
                        .rotation
                        .set_at(time, (degrees * 10.0).round() / 10.0);
                }
            }
            change = Some(Change::Live);
        }
    }
    let hovered = held
        .map(|drag| drag.grip)
        .or_else(|| grip_at(&frame, &corners, response.hover_pos()?));
    if let Some(grip) = hovered {
        ui.ctx().set_cursor_icon(match grip {
            Grip::Move => CursorIcon::Move,
            Grip::Scale => CursorIcon::ResizeNwSe,
            Grip::Rotate => CursorIcon::Alias,
            Grip::Anchor => CursorIcon::Crosshair,
        });
    }
    ui.data_mut(|data| match held {
        Some(held) => {
            data.insert_temp(id, held);
        }
        None => data.remove::<Held>(id),
    });
    paint(ui, &corners, &frame);
    guides.paint(ui, frame.shown);
    if change.is_some() {
        let edit = SetClipLook {
            clip: clip.id,
            look,
        };
        adjust_look(editor, edit, change);
    }
}

/// The selected video clip under the playhead, unless its masks take the handles.
fn moved_clip(editor: &Editor) -> Option<Clip> {
    let sequence = editor.timeline.sequence();
    let playhead = editor.playhead();
    let (_, clip) = editor
        .view
        .selection
        .iter()
        .filter_map(|&id| sequence.clip(id))
        .find(|(track, _)| track.kind == TrackKind::Video)?;
    let shown = (clip.start..clip.end()).contains(&playhead);
    (shown && shown_look(editor, clip).masks.is_empty()).then(|| clip.clone())
}

impl Start {
    fn at(motion: &Motion, time: Ticks) -> Self {
        let pair = |[x, y]: &[Param; 2]| Vec2::new(x.value_at(time), y.value_at(time));
        Self {
            position: pair(&motion.position),
            scale: motion.scale_at(time),
            rotation: motion.rotation.value_at(time),
            anchor: pair(&motion.anchor),
        }
    }
}

fn set_position(motion: &mut Motion, time: Ticks, position: Vec2) {
    let [x, y] = &mut motion.position;
    x.set_at(time, position.x.round());
    y.set_at(time, position.y.round());
}

/// The picture's corners on screen, clockwise from the top left.
fn corners(frame: &PictureFrame) -> [Pos2; 4] {
    let half = frame.fitted * 0.5;
    [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
        .map(|(x, y)| frame.to_screen(Vec2::new(x * half.x, y * half.y)))
}

/// Corners scale before the body moves, so a small picture can still be resized; past
/// the corners, outside the picture, a drag turns it.
fn grip_at(frame: &PictureFrame, corners: &[Pos2; 4], pointer: Pos2) -> Option<Grip> {
    let nearest = corners
        .iter()
        .map(|corner| corner.distance(pointer))
        .fold(f32::INFINITY, f32::min);
    let local = frame.to_picture(pointer);
    let half = frame.fitted * 0.5;
    let inside = local.x.abs() <= half.x && local.y.abs() <= half.y;
    if nearest <= SCALE_REACH {
        Some(Grip::Scale)
    } else if frame.anchor.distance(pointer) <= SCALE_REACH {
        Some(Grip::Anchor)
    } else if inside {
        Some(Grip::Move)
    } else if nearest <= ROTATE_REACH {
        Some(Grip::Rotate)
    } else {
        None
    }
}

fn paint(ui: &egui::Ui, corners: &[Pos2; 4], frame: &PictureFrame) {
    let painter = ui.painter_at(frame.shown.expand(ROTATE_REACH));
    painter.add(Shape::closed_line(
        corners.to_vec(),
        Stroke::new(1.0, OUTLINE),
    ));
    for &corner in corners {
        let square = egui::Rect::from_center_size(corner, Vec2::splat(HANDLE_SIZE));
        painter.rect_filled(square, 0.0, OUTLINE);
        painter.rect_stroke(
            square,
            0.0,
            Stroke::new(1.0, Color32::BLACK),
            egui::StrokeKind::Inside,
        );
    }
    let anchor = frame.anchor;
    let cross = HANDLE_SIZE;
    for axis in [Vec2::X, Vec2::Y] {
        painter.line_segment(
            [anchor - axis * cross, anchor + axis * cross],
            Stroke::new(1.0, OUTLINE),
        );
    }
    painter.circle_stroke(anchor, cross * 0.6, Stroke::new(1.0, OUTLINE));
}
