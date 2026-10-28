use std::sync::Arc;

use egui::{Align2, Color32, FontId, Rect, Stroke, StrokeKind, Vec2};
use timeline::{Clip, Sequence, SwitchAngle, Track};

use crate::editor::Editor;

/// The highest camera the number keys pick.
pub const KEYED_ANGLES: usize = 9;
const ACTIVE: Color32 = Color32::from_rgb(250, 200, 40);
const CUTTING: Color32 = Color32::from_rgb(235, 60, 50);

impl Editor {
    /// Plays `sequence` in the program monitor from the playhead, as the multi-camera
    /// view when that is on.
    pub(crate) fn show_program(&mut self, sequence: Sequence) {
        let shown = self.program_view(sequence);
        if let Some(program) = &mut self.program {
            program.player.set_sequence(shown);
        }
    }

    /// Like [`show_program`](Self::show_program) without seeking, so playback carries on.
    pub(crate) fn mix_program(&mut self, sequence: Sequence) {
        let shown = self.program_view(sequence);
        if let Some(program) = &mut self.program {
            program.player.set_mix(shown);
        }
    }

    fn program_view(&mut self, sequence: Sequence) -> Arc<Sequence> {
        let view = self
            .multicam_view
            .then(|| sequence.multicam_view())
            .flatten();
        self.multicam_layout = view.as_ref().map(|(_, layout)| *layout);
        Arc::new(view.map_or(sequence, |(view, _)| view))
    }

    pub(crate) fn toggle_multicam_view(&mut self) {
        self.multicam_view = !self.multicam_view;
        if self.multicam_view && self.timeline.sequence().multicam_track().is_none() {
            self.notice(
                "Place a multi-camera source sequence in the timeline to cut between its cameras",
            );
        }
        self.show_program(self.timeline.sequence().clone());
    }

    /// Shows camera `angle` in the multi-camera clip under the playhead: from the
    /// playhead on while playing, so cameras are cut live, else for the whole clip.
    pub(crate) fn switch_angle(&mut self, angle: usize) {
        let playhead = self.playhead();
        let sequence = self.timeline.sequence();
        let Some(clip) = sequence.multicam_clip_at(playhead) else {
            return;
        };
        let playing = self
            .program
            .as_ref()
            .is_some_and(|program| program.player.is_playing());
        let rate = sequence.format.frame_rate;
        let frame = rate.frame_start(
            rate.frame_at(playhead, sequence.timebase),
            sequence.timebase,
        );
        let edit = SwitchAngle {
            clip: clip.id,
            angle,
            at: playing.then_some(frame),
        };
        if playing {
            // Cutting must not stop playback with the seek an edit usually brings.
            self.apply_levels(edit);
        } else {
            self.apply(edit);
        }
    }
}

/// Picks the camera the first selected multi-camera clip shows throughout.
pub fn camera_menu(editor: &mut Editor, ui: &mut egui::Ui) {
    let sequence = editor.timeline.sequence();
    let chosen = editor.view.selection.iter().find_map(|&id| {
        let (_, clip) = sequence.clip(id)?;
        Some((clip.id, clip.angle, camera_names(sequence, clip)?))
    });
    let Some((clip, current, names)) = chosen else {
        return;
    };
    ui.menu_button("Multi-Camera", |ui| {
        for (angle, name) in names.iter().enumerate() {
            let text = format!("Camera {} {name}", angle + 1);
            if ui.radio(angle == current, text).clicked() {
                editor.apply(SwitchAngle {
                    clip,
                    angle,
                    at: None,
                });
                ui.close();
            }
        }
    });
}

/// The name of the media on each camera track of `clip`'s multi-camera source.
fn camera_names(sequence: &Sequence, clip: &Clip) -> Option<Vec<String>> {
    let nest = sequence.multicam_of(clip)?;
    let name = |track: &Track| {
        let item = track
            .clips
            .first()
            .and_then(|clip| sequence.media_item(clip.source));
        item.map_or_else(String::new, |item| item.name.clone())
    };
    Some(nest.video.iter().map(name).collect())
}

/// Outlines and names the cameras over the multi-camera view in `area`, the camera
/// showing at the playhead highlighted, and switches to a camera clicked.
pub fn overlay(editor: &mut Editor, ui: &egui::Ui, area: Rect, clicked: Option<egui::Pos2>) {
    let Some(layout) = editor.multicam_layout else {
        return;
    };
    let sequence = editor.timeline.sequence();
    let format = sequence.format;
    let canvas = Vec2::new(format.width as f32, format.height as f32);
    let fit = (area.width() / canvas.x).min(area.height() / canvas.y);
    let frame = Rect::from_center_size(area.center(), canvas * fit);
    let to_screen = |[left, top, width, height]: [f32; 4]| {
        Rect::from_min_size(
            frame.min + Vec2::new(left, top) * frame.size(),
            Vec2::new(width, height) * frame.size(),
        )
    };
    let clip = sequence.multicam_clip_at(editor.playhead());
    let names = clip
        .and_then(|clip| camera_names(sequence, clip))
        .unwrap_or_default();
    let playing = editor
        .program
        .as_ref()
        .is_some_and(|program| program.player.is_playing());
    let painter = ui.painter_at(frame);
    let thin = Stroke::new(1.0, Color32::from_gray(90));
    for angle in 0..layout.angles {
        let cell = to_screen(layout.angle_rect(angle));
        painter.rect_stroke(cell, 0.0, thin, StrokeKind::Inside);
        let name = names.get(angle).map_or("", String::as_str);
        let label = format!("{} {name}", angle + 1);
        let at = cell.left_top() + Vec2::splat(4.0);
        let font = FontId::proportional(11.0);
        painter.text(
            at + Vec2::splat(1.0),
            Align2::LEFT_TOP,
            &label,
            font.clone(),
            Color32::BLACK,
        );
        painter.text(at, Align2::LEFT_TOP, &label, font, Color32::WHITE);
    }
    if let Some(clip) = clip {
        let color = if playing { CUTTING } else { ACTIVE };
        let cell = to_screen(layout.angle_rect(clip.angle));
        painter.rect_stroke(cell, 0.0, Stroke::new(3.0, color), StrokeKind::Inside);
    }
    let picked = clicked.and_then(|pointer| {
        let point = (pointer - frame.min) / frame.size();
        layout.angle_at([point.x, point.y])
    });
    if let Some(angle) = picked {
        editor.switch_angle(angle);
    }
}
