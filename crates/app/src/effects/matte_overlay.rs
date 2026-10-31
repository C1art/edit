use egui::{Color32, CursorIcon, Pos2, Rect, Sense, Shape, Stroke, Vec2};
use timeline::{SetClipLook, Ticks, TrackKind, VideoEffect};

use super::picture_frame::PictureFrame;
use super::{adjust_look, keyframe_time, shown_look, SideTab};
use crate::editor::Editor;
use crate::mixer::Change;

const OUTLINE: Color32 = Color32::from_rgb(120, 220, 255);
const HANDLE_SIZE: f32 = 7.0;
/// How near, in points, the pointer must be to grab a point.
const GRAB_RADIUS: f32 = 8.0;

/// Outlines the selected clip's garbage mattes over the program picture in `area` while
/// the effect controls show; dragging a point moves it.
pub fn matte_overlay(editor: &mut Editor, ui: &egui::Ui, area: Rect) {
    if editor.side_tab != SideTab::EffectControls {
        return;
    }
    let sequence = editor.timeline.sequence();
    let playhead = editor.playhead();
    let Some(clip) = editor
        .view
        .selection
        .iter()
        .filter_map(|&id| sequence.clip(id))
        .find(|(track, clip)| {
            track.kind == TrackKind::Video && (clip.start..clip.end()).contains(&playhead)
        })
        .map(|(_, clip)| clip.clone())
    else {
        return;
    };
    let time = keyframe_time(editor, &clip);
    let mut look = shown_look(editor, &clip);
    let Some(frame) = PictureFrame::new(sequence, &clip, &look, time, area) else {
        return;
    };
    let screen = |point: &[timeline::Param; 2], time: Ticks| {
        let fraction = Vec2::new(point[0].value_at(time), point[1].value_at(time)) / 100.0;
        frame.to_screen((fraction - Vec2::splat(0.5)) * frame.fitted)
    };
    let painter = ui.painter_at(frame.shown);
    let mut change = None;
    let id = ui.make_persistent_id(("matte_overlay", clip.id));
    for (effect_index, effect) in look.effects.iter_mut().enumerate() {
        let VideoEffect::GarbageMatte(matte) = effect else {
            continue;
        };
        let outline: Vec<Pos2> = matte
            .points
            .iter()
            .map(|point| screen(point, time))
            .collect();
        painter.add(Shape::closed_line(
            outline.clone(),
            Stroke::new(1.5, OUTLINE),
        ));
        for (point_index, (point, at)) in matte.points.iter_mut().zip(outline).enumerate() {
            let grip = Rect::from_center_size(at, Vec2::splat(2.0 * GRAB_RADIUS));
            let response = ui
                .interact(grip, id.with((effect_index, point_index)), Sense::drag())
                .on_hover_cursor(CursorIcon::Move);
            if response.dragged() {
                if let Some(pointer) = response.interact_pointer_pos() {
                    let fraction = frame.to_picture(pointer) / frame.fitted + Vec2::splat(0.5);
                    for (axis, param) in point.iter_mut().enumerate() {
                        param.set_at(time, fraction[axis] * 100.0);
                    }
                    change = Some(Change::Live);
                }
            }
            if response.drag_stopped() {
                change = Some(Change::Commit);
            }
            let fill = if response.hovered() || response.dragged() {
                Color32::WHITE
            } else {
                OUTLINE
            };
            let handle = Rect::from_center_size(at, Vec2::splat(HANDLE_SIZE));
            painter.rect_filled(handle, 1.0, fill);
            painter.rect_stroke(
                handle,
                1.0,
                Stroke::new(1.0, Color32::BLACK),
                egui::StrokeKind::Outside,
            );
        }
    }
    if change.is_some() {
        let edit = SetClipLook {
            clip: clip.id,
            look,
        };
        adjust_look(editor, edit, change);
    }
}
