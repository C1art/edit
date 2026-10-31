use timeline::{
    preview, AudioEffect, Clip, ClipId, Edit, SetClipAudioEffects, SetClipPan, SetClipVolume,
};

use super::key_time::seek_to_key;
use super::look::Rows;
use super::{keyframe_time, lane, sound};
use crate::editor::Editor;
use crate::mixer::Change;
use crate::vst::Holder;

pub(super) fn volume_controls(editor: &mut Editor, ui: &mut egui::Ui, clip: &Clip) {
    let time = keyframe_time(editor, clip);
    let (mut volume, mut pan) = match &editor.effects.leveling {
        Some((id, volume, pan)) if *id == clip.id => (volume.clone(), pan.clone()),
        _ => (clip.volume.clone(), clip.pan.clone()),
    };
    let keys = std::mem::take(&mut editor.effects.keys);
    let rows = Rows::new(time, lane(editor, ui, clip), keys);
    let mut output = sound::volume_controls(ui, &mut volume, &mut pan, rows);
    editor.effects.keys = std::mem::take(&mut output.keys);
    seek_to_key(editor, clip, output.seek);
    let Some(change) = output.change else {
        return;
    };
    editor.effects.leveling = match change {
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

/// Plays a change to a clip's sound while it is dragged, and keeps it on release.
pub(crate) fn adjust_levels(editor: &mut Editor, edit: impl Edit, change: Change) {
    match change {
        Change::Live => hear_live(editor, &edit),
        Change::Commit => editor.apply_levels(edit),
    }
}

pub(super) fn audio_effect_controls(editor: &mut Editor, ui: &mut egui::Ui, clip: &Clip) {
    let time = keyframe_time(editor, clip);
    let mut effects = clip_audio_effects(editor, clip);
    let keys = std::mem::take(&mut editor.effects.keys);
    let rows = Rows::new(time, lane(editor, ui, clip), keys);
    let mut output = sound::effect_controls(ui, &mut effects, rows, &mut editor.vst);
    editor.effects.keys = std::mem::take(&mut output.keys);
    seek_to_key(editor, clip, output.seek);
    if let Some(change) = output.change {
        set_clip_audio_effects(editor, clip.id, effects, change);
    }
    if let Some(index) = output.open_editor {
        crate::vst::open_editor(editor, ui.ctx(), Holder::Clip(clip.id), index);
    }
}

/// A clip's audio effects, as they are while being adjusted.
pub(crate) fn clip_audio_effects(editor: &Editor, clip: &Clip) -> Vec<AudioEffect> {
    match &editor.effects.processing {
        Some((id, effects)) if *id == clip.id => effects.clone(),
        _ => clip.audio_effects.clone(),
    }
}

pub(crate) fn set_clip_audio_effects(
    editor: &mut Editor,
    clip: ClipId,
    effects: Vec<AudioEffect>,
    change: Change,
) {
    let edit = SetClipAudioEffects { clip, effects };
    match change {
        Change::Live => {
            hear_live(editor, &edit);
            editor.effects.processing = Some((edit.clip, edit.effects));
        }
        Change::Commit => {
            editor.effects.processing = None;
            editor.apply_levels(edit);
        }
    }
}

/// Plays the program as `edit` would leave it without seeking, so a change to the
/// sound is heard while playback carries on.
pub(super) fn hear_live(editor: &mut Editor, edit: &impl Edit) {
    if let Ok(sequence) = preview(editor.timeline.sequence(), edit) {
        editor.mix_program(sequence);
    }
}
