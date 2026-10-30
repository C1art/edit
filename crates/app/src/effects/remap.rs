use egui::DragValue;
use timeline::{
    AddSpeedKeyframe, Clip, RemoveSpeedKeyframe, RemoveTimeRemap, SetMaintainPitch,
    SetSegmentSpeed, Speed, Ticks,
};

use crate::editor::Editor;
use crate::mixer::{response_state, Change};
use crate::speed_dialog::percent_text;

enum Action {
    Add,
    Seek(Ticks),
    Remove(Ticks),
    Speed(usize, f64, Change),
    Reset,
    MaintainPitch(bool),
}

/// Lists the speed of each stretch of a remapped clip between its keyframes, with the
/// keyframes in between, and keyframes the speed at the playhead.
pub(super) fn controls(editor: &mut Editor, ui: &mut egui::Ui, clip: &Clip) {
    let sequence = editor.timeline.sequence();
    let timecode = |position| sequence.timecode_at(position).to_string();
    let playhead = editor.playhead();
    let dragging = editor.effects.remapping;
    let mut action = None;
    egui::CollapsingHeader::new("Time Remapping")
        .default_open(true)
        .show(ui, |ui| {
            let can_key =
                clip.covers(playhead) && playhead > clip.start && !clip.hold && !clip.reverse;
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(can_key, egui::Button::new("Add Speed Keyframe"))
                    .on_hover_text("Keyframe the speed at the playhead so each side can differ")
                    .clicked()
                {
                    action = Some(Action::Add);
                }
                if clip.remap.is_some() && ui.button("Remove Time Remapping").clicked() {
                    action = Some(Action::Reset);
                }
            });
            let Some(remap) = &clip.remap else {
                ui.horizontal(|ui| {
                    super::controls::label_column(ui, "Speed");
                    ui.label(percent_text(clip.speed));
                });
                return;
            };
            let mut maintain = clip.maintain_pitch;
            if ui
                .checkbox(&mut maintain, "Maintain Audio Pitch")
                .on_hover_text("Keep voices natural as the speed changes, instead of as on tape")
                .changed()
            {
                action = Some(Action::MaintainPitch(maintain));
            }
            let segments = remap.anchors().windows(2).enumerate();
            for (index, pair) in segments {
                let (from, to) = (pair[0].offset, pair[1].offset);
                if to <= Ticks::ZERO || from >= clip.duration {
                    continue;
                }
                if from > Ticks::ZERO {
                    ui.horizontal(|ui| {
                        if ui.link(timecode(clip.start + from)).clicked() {
                            action = Some(Action::Seek(clip.start + from));
                        }
                        if ui.small_button("Remove Keyframe").clicked() {
                            action = Some(Action::Remove(from));
                        }
                    });
                }
                let Some(speed) = remap.speed(index) else {
                    continue;
                };
                let mut percent = match dragging {
                    Some((id, at, percent)) if id == clip.id && at == index => percent,
                    _ => speed.percent(),
                };
                ui.horizontal(|ui| {
                    super::controls::label_column(ui, "Speed");
                    let response = ui.add(
                        DragValue::new(&mut percent)
                            .range(1.0..=10_000.0)
                            .max_decimals(2)
                            .suffix("%"),
                    );
                    if let Some(change) = response_state(&response, None) {
                        action = Some(Action::Speed(index, percent, change));
                    }
                });
            }
        });
    let Some(action) = action else {
        return;
    };
    match action {
        Action::Add => {
            editor.apply(AddSpeedKeyframe {
                clip: clip.id,
                at: playhead,
            });
        }
        Action::Seek(position) => editor.seek_program(position),
        Action::Remove(offset) => {
            editor.apply(RemoveSpeedKeyframe {
                clip: clip.id,
                offset,
            });
        }
        Action::Reset => {
            editor.apply(RemoveTimeRemap { clip: clip.id });
        }
        Action::MaintainPitch(maintain) => {
            editor.apply(SetMaintainPitch {
                clip: clip.id,
                maintain,
            });
        }
        Action::Speed(index, percent, change) => {
            let Some(speed) = Speed::from_basis_points((percent * 100.0).round() as u64) else {
                return;
            };
            let edit = SetSegmentSpeed {
                clip: clip.id,
                index,
                speed,
            };
            editor.effects.remapping = None;
            match change {
                Change::Live => {
                    super::show_live(editor, &edit);
                    editor.effects.remapping = Some((clip.id, index, percent));
                }
                Change::Commit => {
                    editor.apply(edit);
                }
            }
        }
    }
}
