//! Ken Burns boxes over the source monitor: where the frame starts and where it ends on
//! the selected clip's whole picture. Dragging a box moves it, and a corner resizes it
//! about the opposite one, keyframing scale and position at the clip's ends.

use egui::{Align2, Color32, CursorIcon, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use timeline::{Clip, FrameRegion, KenBurns, Look, MoveEnd, SetClipLook, TrackKind};

use super::picture_frame::PictureFrame;
use super::{adjust_look, shown_look};
use crate::editor::Editor;
use crate::mixer::Change;

const START: Color32 = Color32::from_rgb(90, 200, 120);
const END: Color32 = Color32::from_rgb(235, 95, 85);
/// How near, in points, the pointer must be to grab a corner.
const CORNER_REACH: f32 = 8.0;
const HANDLE_SIZE: f32 = 7.0;
/// The narrowest a box can be dragged, in picture pixels.
const NARROWEST: f32 = 16.0;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Grip {
    Move,
    /// Resizes about the opposite corner, on screen, which holds still.
    Corner(Pos2),
}

#[derive(Debug, Clone, Copy)]
struct Held {
    end: MoveEnd,
    grip: Grip,
    from: Pos2,
    region: FrameRegion,
}

/// How picture pixels land on the source monitor.
struct Mapping {
    center: Pos2,
    per_pixel: f32,
    flip: Vec2,
    aspect: f32,
}

impl Editor {
    /// Turns the boxes on or off; on, opens the clip's media in the source monitor at
    /// its first frame to draw them over.
    pub(super) fn set_ken_burns(&mut self, clip: &Clip, on: bool) {
        self.effects.ken_burns = false;
        if !on {
            return;
        }
        let sequence = self.timeline.sequence();
        let format = sequence.format;
        let Some(span) = KenBurns::of(clip, format.frame_rate, sequence.timebase) else {
            self.notice("The clip is a single frame, with nowhere to move");
            return;
        };
        let look = shown_look(self, clip);
        if span.framings(&look.motion, format.width as f32).is_none() {
            self.notice("Ken Burns needs the picture unturned and evenly scaled at both ends");
            return;
        }
        self.effects.ken_burns = true;
        self.open_source(clip.source);
        if let Some(source) = self
            .source
            .as_mut()
            .filter(|source| source.source == clip.source)
        {
            source.show_media(clip.source_in..clip.source_out(), span.first);
        }
    }
}

/// Exchanges where `look` starts and ends; false if it can't.
pub(super) fn swap(editor: &Editor, clip: &Clip, look: &mut Look) -> bool {
    let sequence = editor.timeline.sequence();
    let format = sequence.format;
    KenBurns::of(clip, format.frame_rate, sequence.timebase)
        .is_some_and(|span| span.swap(&mut look.motion, format.width as f32))
}

/// Draws and drags the boxes on the source monitor's picture in `area`, while Ken Burns
/// is on and the monitor shows the selected clip's media.
pub fn ken_burns_overlay(editor: &mut Editor, ui: &egui::Ui, area: Rect) {
    if !editor.effects.ken_burns {
        return;
    }
    let Some(clip) = boxed_clip(editor) else {
        return;
    };
    let sequence = editor.timeline.sequence();
    let format = sequence.format;
    let Some(span) = KenBurns::of(&clip, format.frame_rate, sequence.timebase) else {
        return;
    };
    let mut look = shown_look(editor, &clip);
    let Some(frame) = PictureFrame::new(sequence, &clip, &look, span.first, area) else {
        return;
    };
    let painter = ui.painter_at(area);
    let frame_width = format.width as f32;
    let Some(regions) = span.framings(&look.motion, frame_width) else {
        painter.text(
            area.center_top() + Vec2::new(0.0, 16.0),
            Align2::CENTER_TOP,
            "Ken Burns needs the picture unturned and evenly scaled",
            FontId::proportional(13.0),
            Color32::WHITE,
        );
        return;
    };
    let mapping = Mapping {
        center: area.center(),
        per_pixel: (area.width() / frame.fitted.x).min(area.height() / frame.fitted.y),
        flip: frame.flip,
        aspect: format.height as f32 / frame_width,
    };
    let boxes = [
        (MoveEnd::Start, mapping.rect(regions[0]), regions[0]),
        (MoveEnd::End, mapping.rect(regions[1]), regions[1]),
    ];
    let reach = boxes[0].1.union(boxes[1].1).expand(CORNER_REACH);
    let id = ui.make_persistent_id(("ken_burns", clip.id));
    let response = ui.interact(reach.intersect(area), id, Sense::drag());
    let mut held = ui.data(|data| data.get_temp::<Held>(id));
    if held.is_none() && response.drag_started() {
        held = ui
            .input(|input| input.pointer.press_origin())
            .and_then(|from| {
                let (end, grip, region) = grip_at(&boxes, from)?;
                Some(Held {
                    end,
                    grip,
                    from,
                    region,
                })
            });
    }
    let mut change = None;
    if let Some(drag) = held {
        if !ui.input(|input| input.pointer.primary_down()) {
            held = None;
            if editor.effects.adjusting.is_some() {
                change = Some(Change::Commit);
            }
        } else if let Some(pointer) = ui.input(|input| input.pointer.latest_pos()) {
            let region = mapping.dragged(&drag, pointer);
            span.set(&mut look.motion, drag.end, region, frame_width);
            change = Some(Change::Live);
        }
    }
    let hovered = held
        .map(|drag| drag.grip)
        .or_else(|| Some(grip_at(&boxes, response.hover_pos()?)?.1));
    if let Some(grip) = hovered {
        ui.ctx().set_cursor_icon(cursor(grip, response.hover_pos()));
    }
    ui.data_mut(|data| match held {
        Some(held) => {
            data.insert_temp(id, held);
        }
        None => data.remove::<Held>(id),
    });
    if let Some([start, end]) = span.framings(&look.motion, frame_width) {
        paint(&painter, [mapping.rect(start), mapping.rect(end)]);
    }
    if change.is_some() {
        let edit = SetClipLook {
            clip: clip.id,
            look,
        };
        adjust_look(editor, edit, change);
    }
}

/// The selected video clip of the media the source monitor shows.
fn boxed_clip(editor: &Editor) -> Option<Clip> {
    let media = editor.source.as_ref()?.source;
    let sequence = editor.timeline.sequence();
    editor
        .view
        .selection
        .iter()
        .filter_map(|&id| sequence.clip(id))
        .find(|(track, clip)| track.kind == TrackKind::Video && clip.source == media)
        .map(|(_, clip)| clip.clone())
}

impl Mapping {
    fn rect(&self, region: FrameRegion) -> Rect {
        let center = self.center + Vec2::from(region.center) * self.flip * self.per_pixel;
        let size = Vec2::new(region.width, region.width * self.aspect) * self.per_pixel;
        Rect::from_center_size(center, size)
    }

    fn dragged(&self, drag: &Held, pointer: Pos2) -> FrameRegion {
        let to_picture = |screen: Vec2| screen / self.per_pixel.max(f32::EPSILON) * self.flip;
        match drag.grip {
            Grip::Move => {
                let moved = Vec2::from(drag.region.center) + to_picture(pointer - drag.from);
                FrameRegion {
                    center: moved.into(),
                    width: drag.region.width,
                }
            }
            Grip::Corner(still) => {
                let diagonal = drag.from - still;
                let along = (pointer - still).dot(diagonal) / diagonal.length_sq().max(1.0);
                let width = (drag.region.width * along).max(NARROWEST);
                let ratio = width / drag.region.width.max(f32::EPSILON);
                let center = still + diagonal * ratio * 0.5;
                FrameRegion {
                    center: to_picture(center - self.center).into(),
                    width,
                }
            }
        }
    }
}

/// Corners before bodies, and the end box before the start, as it's drawn on top.
fn grip_at(
    boxes: &[(MoveEnd, Rect, FrameRegion); 2],
    pointer: Pos2,
) -> Option<(MoveEnd, Grip, FrameRegion)> {
    let corner = boxes.iter().rev().find_map(|&(end, rect, region)| {
        let corner = corners(rect)
            .into_iter()
            .find(|corner| corner.distance(pointer) <= CORNER_REACH)?;
        let opposite = rect.center() + (rect.center() - corner);
        Some((end, Grip::Corner(opposite), region))
    });
    corner.or_else(|| {
        boxes
            .iter()
            .rev()
            .find(|(_, rect, _)| rect.contains(pointer))
            .map(|&(end, _, region)| (end, Grip::Move, region))
    })
}

fn cursor(grip: Grip, pointer: Option<Pos2>) -> CursorIcon {
    match (grip, pointer) {
        (Grip::Corner(still), Some(pointer)) => {
            let toward = pointer - still;
            if toward.x * toward.y >= 0.0 {
                CursorIcon::ResizeNwSe
            } else {
                CursorIcon::ResizeNeSw
            }
        }
        (Grip::Corner(_), None) => CursorIcon::ResizeNwSe,
        (Grip::Move, _) => CursorIcon::Move,
    }
}

fn corners(rect: Rect) -> [Pos2; 4] {
    [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ]
}

fn paint(painter: &egui::Painter, [start, end]: [Rect; 2]) {
    let travel = end.center() - start.center();
    if travel.length() > HANDLE_SIZE {
        painter.arrow(start.center(), travel, Stroke::new(1.5, Color32::WHITE));
    }
    for (rect, color, label) in [(start, START, "Start"), (end, END, "End")] {
        painter.rect_stroke(rect, 0.0, Stroke::new(2.0, color), egui::StrokeKind::Inside);
        for corner in corners(rect) {
            let handle = Rect::from_center_size(corner, Vec2::splat(HANDLE_SIZE));
            painter.rect_filled(handle, 0.0, color);
            painter.rect_stroke(
                handle,
                0.0,
                Stroke::new(1.0, Color32::BLACK),
                egui::StrokeKind::Inside,
            );
        }
        painter.text(
            rect.left_top() + Vec2::new(6.0, 5.0),
            Align2::LEFT_TOP,
            label,
            FontId::proportional(12.0),
            color,
        );
    }
}
