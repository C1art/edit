mod generated;
mod look;
mod sound;
mod transitions;

use std::sync::Arc;

use egui::RichText;
use timeline::{
    preview, AudioEffect, Clip, ClipId, Edit, Generator, Look, Param, Sequence,
    SetClipAudioEffects, SetClipLook, SetClipVolume, SetGenerator, SourceId, Ticks, TrackKind,
    TrackRef,
};

use crate::editor::Editor;
use crate::mixer::Change;

pub use transitions::apply_default_transition;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SideTab {
    #[default]
    EffectControls,
    AudioMixer,
    Scopes,
}

/// The look, volume or audio effect being dragged, or the title being typed, shown live
/// and committed on release.
#[derive(Debug, Default)]
pub struct EffectControls {
    adjusting: Option<(ClipId, Look)>,
    leveling: Option<(ClipId, Param)>,
    processing: Option<(ClipId, Vec<AudioEffect>)>,
    drawing: Option<(SourceId, Generator)>,
}

pub fn ui(editor: &mut Editor, ui: &mut egui::Ui) {
    let sequence = editor.timeline.sequence();
    // Linked video and audio are selected together and adjusted side by side.
    let selected: Vec<(TrackRef, &Clip)> = editor
        .view
        .selection
        .iter()
        .filter_map(|&id| sequence.clip(id))
        .collect();
    let first = |kind: TrackKind| {
        selected
            .iter()
            .find(|(track, _)| track.kind == kind)
            .map(|&(track, clip)| (track, clip.clone()))
    };
    let (picture, sound) = (first(TrackKind::Video), first(TrackKind::Audio));
    let Some((track, clip)) = picture.clone().or_else(|| sound.clone()) else {
        ui.label("Select a clip to adjust its motion, opacity, crop, effects and transitions.");
        return;
    };
    ui.label(RichText::new(clip_name(sequence, &clip)).strong());
    egui::ScrollArea::vertical().show(ui, |ui| {
        if let Some((_, clip)) = &picture {
            picture_controls(editor, ui, clip);
        }
        if let Some((_, clip)) = &sound {
            volume_controls(editor, ui, clip);
            audio_effect_controls(editor, ui, clip);
        }
        transitions::controls(editor, ui, track, &clip);
    });
}

fn picture_controls(editor: &mut Editor, ui: &mut egui::Ui, clip: &Clip) {
    let time = keyframe_time(editor, clip);
    let mut look = match &editor.effects.adjusting {
        Some((id, look)) if *id == clip.id => look.clone(),
        _ => clip.look.clone(),
    };
    let mut drawing = editor
        .timeline
        .sequence()
        .media_item(clip.source)
        .and_then(|item| item.generator.clone())
        .map(|generator| match &editor.effects.drawing {
            Some((id, drawing)) if *id == clip.source => drawing.clone(),
            _ => generator,
        });
    let drawing_change = drawing
        .as_mut()
        .and_then(|generator| generated::controls(ui, generator));
    let output = look::controls(ui, &mut look, time);
    seek_to_key(editor, clip, output.seek);
    if let (Some(generator), Some(change)) = (drawing, drawing_change) {
        let edit = SetGenerator {
            source: clip.source,
            generator,
        };
        editor.effects.drawing = None;
        match change {
            Change::Live => {
                show_live(editor, &edit);
                editor.effects.drawing = Some((edit.source, edit.generator));
            }
            Change::Commit => {
                editor.apply(edit);
            }
        }
    }
    let edit = SetClipLook {
        clip: clip.id,
        look,
    };
    match output.change {
        Some(Change::Live) => {
            show_live(editor, &edit);
            editor.effects.adjusting = Some((edit.clip, edit.look));
        }
        Some(Change::Commit) => {
            editor.effects.adjusting = None;
            editor.apply(edit);
        }
        None => {}
    }
}

fn volume_controls(editor: &mut Editor, ui: &mut egui::Ui, clip: &Clip) {
    let time = keyframe_time(editor, clip);
    let mut volume = match &editor.effects.leveling {
        Some((id, volume)) if *id == clip.id => volume.clone(),
        _ => clip.volume.clone(),
    };
    let output = sound::volume_controls(ui, &mut volume, time);
    seek_to_key(editor, clip, output.seek);
    let edit = SetClipVolume {
        clip: clip.id,
        volume,
    };
    match output.change {
        Some(Change::Live) => {
            hear_live(editor, &edit);
            editor.effects.leveling = Some((edit.clip, edit.volume));
        }
        Some(Change::Commit) => {
            editor.effects.leveling = None;
            editor.apply_levels(edit);
        }
        None => {}
    }
}

fn audio_effect_controls(editor: &mut Editor, ui: &mut egui::Ui, clip: &Clip) {
    let time = keyframe_time(editor, clip);
    let mut effects = match &editor.effects.processing {
        Some((id, effects)) if *id == clip.id => effects.clone(),
        _ => clip.audio_effects.clone(),
    };
    let output = sound::effect_controls(ui, &mut effects, time);
    seek_to_key(editor, clip, output.seek);
    let edit = SetClipAudioEffects {
        clip: clip.id,
        effects,
    };
    match output.change {
        Some(Change::Live) => {
            hear_live(editor, &edit);
            editor.effects.processing = Some((edit.clip, edit.effects));
        }
        Some(Change::Commit) => {
            editor.effects.processing = None;
            editor.apply_levels(edit);
        }
        None => {}
    }
}

/// The source time keyframes go to: under the playhead, kept within the clip.
fn keyframe_time(editor: &Editor, clip: &Clip) -> Ticks {
    clip.source_time(editor.playhead().clamp(clip.start, clip.end() - Ticks(1)))
}

fn seek_to_key(editor: &mut Editor, clip: &Clip, key: Option<Ticks>) {
    if let Some(key) = key {
        let position = clip.position_of(key);
        editor.seek_program(position.clamp(clip.start, clip.end() - Ticks(1)));
    }
}

/// Shows the program as `edit` would leave it, without adding to the undo history.
fn show_live(editor: &mut Editor, edit: &impl Edit) {
    if let Ok(sequence) = preview(editor.timeline.sequence(), edit) {
        if let Some(program) = &mut editor.program {
            program.player.set_sequence(Arc::new(sequence));
        }
    }
}

/// Plays the program as `edit` would leave it without seeking, so a change to the
/// sound is heard while playback carries on.
fn hear_live(editor: &mut Editor, edit: &impl Edit) {
    if let Ok(sequence) = preview(editor.timeline.sequence(), edit) {
        if let Some(program) = &mut editor.program {
            program.player.set_mix(Arc::new(sequence));
        }
    }
}

fn clip_name<'a>(sequence: &'a Sequence, clip: &Clip) -> &'a str {
    sequence
        .media_item(clip.source)
        .map_or("Missing media", |item| item.name.as_str())
}
