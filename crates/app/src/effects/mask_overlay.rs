use egui::{Color32, CursorIcon, Pos2, Rect, Sense, Shape, Stroke, Vec2};
use timeline::{Clip, Look, Mask, MaskShape, Sequence, SetClipLook, Ticks, TrackKind, VideoEffect};

use super::{adjust_look, keyframe_time, shown_look, SideTab};
use crate::editor::Editor;
use crate::mixer::Change;

const OUTLINE: Color32 = Color32::from_rgb(255, 205, 60);
const HANDLE_RADIUS: f32 = 3.5;
/// How near, in points, the pointer must be to grab a handle.
const GRAB_RADIUS: f32 = 8.0;
const ELLIPSE_STEPS: usize = 72;

/// What a drag on the program picture holds of a mask.
#[derive(Debug, Clone, Copy)]
enum Grip {
    /// The body, grabbed this far from the mask's center in picture pixels.
    Body(Vec2),
    /// The handle on a side across the width (0) or the height (1).
    Side(usize),
}

/// Outlines the selected clip's masks over the program picture in `area` while the
/// effect controls show; dragging a mask moves it and dragging a side handle resizes it.
pub fn mask_overlay(editor: &mut Editor, ui: &egui::Ui, area: Rect) {
    if editor.side_tab != SideTab::EffectControls {
        return;
    }
    let Some(clip) = masked_clip(editor) else {
        return;
    };
    let time = keyframe_time(editor, &clip);
    let mut look = shown_look(editor, &clip);
    let sequence = editor.timeline.sequence();
    let Some(frame) = PictureFrame::new(sequence, &clip, &look, time, area) else {
        return;
    };
    let id = ui.make_persistent_id(("mask_overlay", clip.id));
    let response = ui.interact(frame.shown, id, Sense::drag());
    let shapes: Vec<MaskGeometry> = look
        .masks
        .iter()
        .map(|mask| MaskGeometry::new(mask, time, frame.fitted))
        .collect();
    let mut change = None;
    if response.drag_started() {
        let origin = ui.input(|input| input.pointer.press_origin());
        if let Some(grip) = origin.and_then(|origin| grab(&frame, &shapes, origin)) {
            ui.data_mut(|data| data.insert_temp(id, grip));
        }
    }
    let held = ui.data(|data| data.get_temp::<(usize, Grip)>(id));
    if let Some((index, grip)) = held {
        if response.drag_stopped() {
            ui.data_mut(|data| data.remove::<(usize, Grip)>(id));
            change = Some(Change::Commit);
        } else if let (Some(pointer), Some(mask), Some(shape)) = (
            response.interact_pointer_pos(),
            look.masks.get_mut(index),
            shapes.get(index),
        ) {
            drag(
                mask,
                shape,
                grip,
                frame.to_picture(pointer),
                frame.fitted,
                time,
            );
            change = Some(Change::Live);
        }
    } else if let Some(pointer) = response.hover_pos() {
        if grab(&frame, &shapes, pointer).is_some() {
            ui.ctx().set_cursor_icon(CursorIcon::Move);
        }
    }
    paint(ui, &frame, &look, time);
    if change.is_some() {
        let edit = SetClipLook {
            clip: clip.id,
            look,
        };
        adjust_look(editor, edit, change);
    }
}

/// The selected video clip under the playhead, if it has masks.
fn masked_clip(editor: &Editor) -> Option<Clip> {
    let sequence = editor.timeline.sequence();
    let playhead = editor.playhead();
    let (_, clip) = editor
        .view
        .selection
        .iter()
        .filter_map(|&id| sequence.clip(id))
        .find(|(track, _)| track.kind == TrackKind::Video)?;
    let shown = (clip.start..clip.end()).contains(&playhead);
    (shown && !shown_look(editor, clip).masks.is_empty()).then(|| clip.clone())
}

/// The topmost handle or mask body at `pointer`, handles first so a small mask can
/// still be resized.
fn grab(frame: &PictureFrame, shapes: &[MaskGeometry], pointer: Pos2) -> Option<(usize, Grip)> {
    let handle = shapes.iter().enumerate().rev().find_map(|(index, shape)| {
        shape
            .handles()
            .into_iter()
            .find(|&(_, local)| {
                frame.to_screen(shape.to_picture(local)).distance(pointer) <= GRAB_RADIUS
            })
            .map(|(axis, _)| (index, Grip::Side(axis)))
    });
    let picture = frame.to_picture(pointer);
    handle.or_else(|| {
        shapes
            .iter()
            .enumerate()
            .rev()
            .find(|(_, shape)| shape.contains(shape.to_local(picture)))
            .map(|(index, shape)| (index, Grip::Body(picture - shape.center)))
    })
}

fn drag(mask: &mut Mask, shape: &MaskGeometry, grip: Grip, to: Vec2, fitted: Vec2, time: Ticks) {
    match grip {
        Grip::Body(offset) => {
            let center = to - offset;
            for (axis, param) in mask.center.iter_mut().enumerate() {
                param.set_at(time, (center[axis] / fitted[axis] + 0.5) * 100.0);
            }
        }
        Grip::Side(axis) => {
            let half = shape.to_local(to)[axis].abs();
            mask.size[axis].set_at(time, 2.0 * half / fitted[axis] * 100.0);
        }
    }
}

fn paint(ui: &egui::Ui, frame: &PictureFrame, look: &Look, time: Ticks) {
    let painter = ui.painter_at(frame.shown);
    let stroke = Stroke::new(1.5, OUTLINE);
    for mask in &look.masks {
        let shape = MaskGeometry::new(mask, time, frame.fitted);
        let screen = |local: Vec2| frame.to_screen(shape.to_picture(local));
        let outline: Vec<Pos2> = match mask.shape {
            MaskShape::Ellipse => (0..ELLIPSE_STEPS)
                .map(|step| {
                    let angle = step as f32 / ELLIPSE_STEPS as f32 * std::f32::consts::TAU;
                    screen(Vec2::new(angle.cos(), angle.sin()) * shape.half)
                })
                .collect(),
            MaskShape::Rectangle => [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
                .map(|(x, y)| screen(Vec2::new(x, y) * shape.half))
                .to_vec(),
        };
        painter.add(Shape::closed_line(outline, stroke));
        for (_, local) in shape.handles() {
            painter.circle(
                screen(local),
                HANDLE_RADIUS,
                OUTLINE,
                Stroke::new(1.0, Color32::BLACK),
            );
        }
    }
}

/// Where the clip's picture lands on screen: its center and the screen vectors along
/// one pixel of its width and height, as fitted to the sequence frame.
struct PictureFrame {
    shown: Rect,
    center: Pos2,
    x: Vec2,
    y: Vec2,
    /// The picture fitted to the sequence frame, in sequence pixels.
    fitted: Vec2,
}

impl PictureFrame {
    fn new(sequence: &Sequence, clip: &Clip, look: &Look, time: Ticks, area: Rect) -> Option<Self> {
        let format = sequence.format;
        let canvas = Vec2::new(format.width as f32, format.height as f32);
        let source = sequence
            .media_item(clip.source)?
            .video
            .map_or(canvas, |video| {
                Vec2::new(video.width as f32, video.height as f32)
            });
        let fit = (canvas.x / source.x).min(canvas.y / source.y);
        let to_screen = (area.width() / canvas.x).min(area.height() / canvas.y);
        let motion = &look.motion;
        let scale = motion.scale.value_at(time) / 100.0 * to_screen;
        let (sin, cos) = motion.rotation.value_at(time).to_radians().sin_cos();
        // Flipping twice leaves the picture as it was.
        let flip = |effect: VideoEffect| {
            let flips = look.effects.iter().filter(|&used| *used == effect).count();
            if flips % 2 == 1 {
                -1.0
            } else {
                1.0
            }
        };
        let position = Vec2::new(
            motion.position[0].value_at(time),
            motion.position[1].value_at(time),
        );
        Some(Self {
            shown: Rect::from_center_size(area.center(), canvas * to_screen),
            center: area.center() + position * to_screen,
            x: Vec2::new(cos, sin) * scale * flip(VideoEffect::HorizontalFlip),
            y: Vec2::new(-sin, cos) * scale * flip(VideoEffect::VerticalFlip),
            fitted: source * fit,
        })
    }

    fn to_screen(&self, picture: Vec2) -> Pos2 {
        self.center + self.x * picture.x + self.y * picture.y
    }

    /// The axes stay perpendicular, so projecting onto each inverts `to_screen`.
    fn to_picture(&self, screen: Pos2) -> Vec2 {
        let offset = screen - self.center;
        Vec2::new(
            offset.dot(self.x) / self.x.length_sq().max(f32::EPSILON),
            offset.dot(self.y) / self.y.length_sq().max(f32::EPSILON),
        )
    }
}

/// A mask at one time, in picture pixels from the picture's center.
struct MaskGeometry {
    shape: MaskShape,
    center: Vec2,
    half: Vec2,
    sin: f32,
    cos: f32,
}

impl MaskGeometry {
    fn new(mask: &Mask, time: Ticks, fitted: Vec2) -> Self {
        let fraction = |param: &timeline::Param| param.value_at(time) / 100.0;
        let [x, y] = mask.center.each_ref().map(fraction);
        let [width, height] = mask.size.each_ref().map(fraction);
        let (sin, cos) = mask.rotation.value_at(time).to_radians().sin_cos();
        Self {
            shape: mask.shape,
            center: (Vec2::new(x, y) - Vec2::splat(0.5)) * fitted,
            half: Vec2::new(width, height) * fitted * 0.5,
            sin,
            cos,
        }
    }

    fn to_picture(&self, local: Vec2) -> Vec2 {
        self.center
            + Vec2::new(
                local.x * self.cos - local.y * self.sin,
                local.x * self.sin + local.y * self.cos,
            )
    }

    fn to_local(&self, picture: Vec2) -> Vec2 {
        let offset = picture - self.center;
        Vec2::new(
            offset.x * self.cos + offset.y * self.sin,
            -offset.x * self.sin + offset.y * self.cos,
        )
    }

    fn contains(&self, local: Vec2) -> bool {
        let half = self.half.max(Vec2::splat(f32::EPSILON));
        match self.shape {
            MaskShape::Ellipse => (local / half).length_sq() <= 1.0,
            MaskShape::Rectangle => local.x.abs() <= half.x && local.y.abs() <= half.y,
        }
    }

    /// The side handles and the axis each resizes.
    fn handles(&self) -> [(usize, Vec2); 4] {
        let Vec2 { x, y } = self.half;
        [
            (0, Vec2::new(x, 0.0)),
            (0, Vec2::new(-x, 0.0)),
            (1, Vec2::new(0.0, y)),
            (1, Vec2::new(0.0, -y)),
        ]
    }
}
