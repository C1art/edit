use timeline::{Clip, PinnedEnds, SetPinnedEnds, Ticks};

use crate::editor::Editor;

/// Responsive Design – Time: how long the clip's opening and closing animations last,
/// so trimming carries them with its ends instead of cutting them off.
pub(super) fn controls(editor: &mut Editor, ui: &mut egui::Ui, clip: &Clip) {
    let sequence = editor.timeline.sequence();
    let (timebase, rate) = (sequence.timebase, sequence.format.frame_rate);
    let format = (timebase, rate, sequence.time_display);
    let speed = clip.rate();
    let to_frames = |source: Ticks| rate.frame_at(speed.sequence_span(source), timebase);
    let to_source = |frames: i64| speed.source_span(rate.frame_start(frames, timebase));
    let longest = rate.frame_at(clip.duration, timebase);
    let id = ui.make_persistent_id(("pinned ends", clip.id));
    let held = ui.data(|data| data.get_temp::<[i64; 2]>(id));
    let mut frames = held.unwrap_or([clip.pinned.intro, clip.pinned.outro].map(to_frames));
    let (mut dragging, mut done) = (false, false);
    egui::CollapsingHeader::new("Responsive Design – Time")
        .default_open(!clip.pinned.is_unpinned())
        .show(ui, |ui| {
            let rows = [
                (
                    "Intro",
                    "Keyframes this near the start move with it when it's trimmed",
                ),
                (
                    "Outro",
                    "Keyframes this near the end move with it when it's trimmed",
                ),
            ];
            for ((label, hint), length) in rows.into_iter().zip(&mut frames) {
                ui.horizontal(|ui| {
                    super::controls::label_column(ui, label);
                    let field = crate::timecode_field::frames(length, format).range(0..=longest);
                    let response = ui.add(field).on_hover_text(hint);
                    dragging |= response.dragged();
                    done |= response.drag_stopped() || (response.changed() && !response.dragged());
                });
            }
            ui.weak("Animation presets set these for the moves they add.");
        });
    ui.data_mut(|data| {
        if dragging && !done {
            data.insert_temp(id, frames);
        } else {
            data.remove::<[i64; 2]>(id);
        }
    });
    let [intro, outro] = frames.map(to_source);
    if done {
        editor.apply(SetPinnedEnds {
            clip: clip.id,
            pinned: PinnedEnds { intro, outro },
        });
    }
}
