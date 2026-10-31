use egui::{RichText, Slider};
use playback::PlayRate;
use timeline::{Clip, ClipId, Gain, Param, SetClipPan, SetClipVolume, Ticks, TrackRef};

use super::{meter_bar, response_state, Change, FADER_RANGE, METER_FLOOR, RECORDING, STRIP_HEIGHT};
use crate::editor::Editor;
use crate::effects::adjust_levels;

/// The clip fader or knob being moved, and whether moves made while playing are
/// written as keyframes.
#[derive(Debug, Default)]
pub struct ClipMixer {
    adjusting: Option<(ClipId, Param, Param)>,
    write_keyframes: bool,
}

/// Premiere's Audio Clip Mixer: for each audio track, the volume and balance of the
/// clip under the playhead.
pub fn ui(editor: &mut Editor, ui: &mut egui::Ui) {
    super::meter(editor, ui);
    ui.checkbox(&mut editor.mixer.clips.write_keyframes, "Write Keyframes")
        .on_hover_text("Moving a fader or knob during playback keyframes the clip");
    let tracks = editor.timeline.sequence().audio.len();
    egui::ScrollArea::horizontal().show(ui, |ui| {
        ui.horizontal_top(|ui| {
            for index in 0..tracks {
                strip(editor, ui, TrackRef::audio(index));
                ui.add_space(6.0);
            }
        });
    });
}

fn strip(editor: &mut Editor, ui: &mut egui::Ui, track_ref: TrackRef) {
    let playhead = editor.playhead();
    commit_left_clip(editor, track_ref, playhead);
    let playing = editor.program.as_ref().is_some_and(|program| {
        program.player.is_playing() && program.player.rate() > PlayRate::STOPPED
    });
    let sequence = editor.timeline.sequence();
    let Some(track) = sequence.track(track_ref) else {
        return;
    };
    let name = track.name.clone().unwrap_or_else(|| track_ref.to_string());
    let clip: Option<Clip> = track.clip_at(playhead).cloned();
    let clip_name = clip.as_ref().map(|clip| {
        sequence
            .media_item(clip.source)
            .map_or("Missing media", |item| item.name.as_str())
            .to_owned()
    });
    let meter = editor.mixer.meters.get(track_ref.index).copied();
    let state = &mut editor.mixer.clips;
    let (mut volume, mut pan) = match (&clip, &state.adjusting) {
        (Some(clip), Some((id, volume, pan))) if *id == clip.id => (volume.clone(), pan.clone()),
        (Some(clip), _) => (clip.volume.clone(), clip.pan.clone()),
        (None, _) => (Param::default(), Param::default()),
    };
    let at = clip
        .as_ref()
        .map_or(playhead, |clip| clip.source_time(playhead));
    let writing = state.write_keyframes && playing;
    let mut changed = None;
    ui.vertical(|ui| {
        ui.set_width(60.0);
        let mut title = RichText::new(&name).strong();
        if writing && clip.is_some() {
            title = title.color(RECORDING);
        }
        ui.add(egui::Label::new(title).truncate());
        let shown = clip_name.as_deref().unwrap_or("—");
        ui.add(egui::Label::new(RichText::new(shown).small()).truncate())
            .on_hover_text(shown);
        ui.add_enabled_ui(clip.is_some(), |ui| {
            let mut db = volume.value_at(at).max(*FADER_RANGE.start());
            ui.horizontal(|ui| {
                ui.spacing_mut().slider_width = STRIP_HEIGHT;
                meter_bar(ui, meter.unwrap_or([METER_FLOOR; 2]));
                let fader = ui.add(
                    Slider::new(&mut db, FADER_RANGE)
                        .update_while_editing(false)
                        .vertical()
                        .show_value(false)
                        .step_by(0.1),
                );
                let fader = fader.on_hover_text("Double-click for 0 dB");
                if fader.double_clicked() {
                    db = 0.0;
                }
                // The slider snaps a level between its steps and calls that a change,
                // which a level following keyframes nearly always is.
                let handled = fader.dragged()
                    || fader.is_pointer_button_down_on()
                    || fader.drag_stopped()
                    || fader.clicked()
                    || fader.has_focus();
                if let Some(change) = response_state(&fader, None).filter(|_| handled) {
                    let db = if db <= *FADER_RANGE.start() {
                        Gain::SILENT.db()
                    } else {
                        db
                    };
                    set(&mut volume, at, db, writing);
                    changed = Some(change);
                }
            });
            let label = if db <= *FADER_RANGE.start() {
                "-inf".to_owned()
            } else {
                format!("{db:+.1}")
            };
            ui.monospace(label).on_hover_text("dB");
            let mut balance = pan.value_at(at);
            ui.spacing_mut().slider_width = 56.0;
            let knob = ui
                .add(
                    Slider::new(&mut balance, -100.0..=100.0)
                        .update_while_editing(false)
                        .show_value(false),
                )
                .on_hover_text(format!("Balance {balance:.0} (double-click to centre)"));
            if knob.double_clicked() {
                balance = 0.0;
            }
            if let Some(change) = response_state(&knob, None) {
                set(&mut pan, at, balance.round(), writing);
                changed = changed.max(Some(change));
            }
        });
    });
    let (Some(clip), Some(change)) = (clip, changed) else {
        return;
    };
    editor.mixer.clips.adjusting = match change {
        Change::Live => Some((clip.id, volume.clone(), pan.clone())),
        Change::Commit => None,
    };
    if volume != clip.volume {
        let edit = SetClipVolume {
            clip: clip.id,
            volume,
        };
        adjust_levels(editor, edit, change);
    }
    if pan != clip.pan {
        adjust_levels(editor, SetClipPan { clip: clip.id, pan }, change);
    }
}

/// Commits what a drag wrote into a clip once the playhead has moved on from it into the
/// next, as the drag goes on adjusting that one instead.
fn commit_left_clip(editor: &mut Editor, track_ref: TrackRef, playhead: Ticks) {
    let sequence = editor.timeline.sequence();
    let Some((id, ..)) = &editor.mixer.clips.adjusting else {
        return;
    };
    let Some((track, left)) = sequence.clip(*id) else {
        return;
    };
    let under_playhead = sequence
        .track(track)
        .and_then(|track| track.clip_at(playhead))
        .is_some_and(|clip| clip.id == *id);
    if track != track_ref || under_playhead {
        return;
    }
    let (committed_volume, committed_pan) = (left.volume.clone(), left.pan.clone());
    let Some((clip, volume, pan)) = editor.mixer.clips.adjusting.take() else {
        return;
    };
    if volume != committed_volume {
        adjust_levels(editor, SetClipVolume { clip, volume }, Change::Commit);
    }
    if pan != committed_pan {
        adjust_levels(editor, SetClipPan { clip, pan }, Change::Commit);
    }
}

/// Sets `param` at `at`: as a keyframe there while writing, and otherwise as the
/// value of an unanimated one or its keyframe at that time.
fn set(param: &mut Param, at: Ticks, value: f32, writing: bool) {
    if writing && !param.is_animated() {
        param.toggle_animation(at);
    }
    param.set_at(at, value);
}
