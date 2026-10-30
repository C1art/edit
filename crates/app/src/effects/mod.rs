mod blur_echo;
mod clip_wipes;
mod color;
mod controls;
mod crop_overlay;
mod curves;
mod detail;
mod edges;
mod effect_stack;
mod generate;
mod generated;
mod graphic_layers;
mod hue_curves;
mod image;
mod ken_burns;
mod key_selection;
mod key_time;
mod keying;
mod lane;
mod layer_align;
mod layer_animation;
mod layered;
mod lighting;
mod look;
mod lumetri;
mod mask_overlay;
mod masks;
mod matte_overlay;
mod motion_overlay;
mod motion_path;
mod motion_snap;
mod optics;
mod paint;
mod picture_frame;
mod reframe;
mod remap;
mod responsive;
mod secondary;
mod shift;
mod simple_text;
mod sound;
mod sound_more;
mod stabilizer;
mod text_style_row;
mod together;
mod tone;
mod transitions;
mod video_effects;

use std::path::PathBuf;

use egui::RichText;
use timeline::{
    preview, AudioEffect, Clip, ClipId, Edit, Generator, Look, Param, Sequence,
    SetClipAudioEffects, SetClipLook, SetClipPan, SetClipVolume, SetGenerator, SourceId, TrackKind,
    TrackRef, VideoEffect,
};

use crate::editor::Editor;
use crate::mixer::Change;
use crate::project_files::PathPurpose;
use color::ToneState;
use key_selection::KeySelection;
use lane::Lane;
use look::Rows;

pub use color::LutSlot;
pub use ken_burns::ken_burns_overlay;
pub(crate) use key_selection::KeyAction;
pub(crate) use key_time::keyframe_time;
use key_time::seek_to_key;
pub use lumetri::ui as lumetri_ui;
pub use mask_overlay::mask_overlay;
pub use matte_overlay::matte_overlay;
pub use motion_overlay::motion_overlay;
pub(crate) use sound::track_effect_controls;
pub(crate) use sound::{category as audio_category, CATEGORIES as AUDIO_CATEGORIES};
pub(crate) use text_style_row::StyleRequest;
pub use transitions::{
    add_transition, apply_default_transition, apply_default_transitions_to_selection,
    apply_transition, remove_transitions_from_selection,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SideTab {
    #[default]
    EffectControls,
    LumetriColor,
    AudioMixer,
    ClipMixer,
    EssentialSound,
    Scopes,
    Captions,
}

/// The look, volume or audio effect being dragged, or the title being typed, shown live
/// and committed on release.
#[derive(Debug, Default)]
pub struct EffectControls {
    adjusting: Option<(ClipId, Look)>,
    leveling: Option<(ClipId, Param, Param)>,
    processing: Option<(ClipId, Vec<AudioEffect>)>,
    drawing: Option<(SourceId, Generator)>,
    /// The clip, segment and speed in percent of a time remapping drag.
    remapping: Option<(ClipId, usize, f64)>,
    keys: KeySelection,
    /// The property of several clips being dragged together, and its value.
    together: Option<(together::Property, f32)>,
    /// Whether the Crop section is open, which turns the program picture's handles to
    /// cropping.
    cropping: bool,
    /// Whether the source monitor shows where the selected clip's picture starts and ends.
    ken_burns: bool,
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
    let ids: Vec<ClipId> = selected.iter().map(|(_, clip)| clip.id).collect();
    let Some(&(track, ref clip)) = picture.as_ref().or(sound.as_ref()) else {
        ui.label("Select a clip to adjust its motion, opacity, crop, effects and transitions.");
        return;
    };
    let name = clip_name(sequence, clip).to_owned();
    // A long clip name is cut short rather than widening the panel.
    let row = egui::vec2(ui.available_width(), ui.spacing().interact_size.y);
    ui.allocate_ui_with_layout(
        row,
        egui::Layout::right_to_left(egui::Align::Center),
        |ui| {
            let save = ui.small_button("Save Preset…").on_hover_text(
                "Keep this clip's effects to apply to others from the Effects panel",
            );
            if save.clicked() {
                editor.name_new_preset(clip.id);
            }
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.add(egui::Label::new(RichText::new(name).strong()).truncate());
            });
        },
    );
    egui::ScrollArea::vertical().show(ui, |ui| {
        // Sections nest a level or two deep, and the panel is often narrow.
        ui.spacing_mut().indent = 10.0;
        together::controls(editor, ui, &ids);
        if let Some((_, clip)) = &picture {
            picture_controls(editor, ui, clip, Sections::All);
            if !editor.simple() || clip.remap.is_some() {
                remap::controls(editor, ui, clip);
            }
            if !editor.simple() {
                responsive::controls(editor, ui, clip);
            }
        }
        if let Some((_, clip)) = &sound {
            volume_controls(editor, ui, clip);
            audio_effect_controls(editor, ui, clip);
        }
        transitions::controls(editor, ui, track, clip);
    });
    editor.effects.keys.end_frame(ui, clip.id);
}

/// Which of a clip's picture controls a panel shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sections {
    All,
    Color,
}

fn picture_controls(editor: &mut Editor, ui: &mut egui::Ui, clip: &Clip, sections: Sections) {
    let time = keyframe_time(editor, clip);
    let mut look = shown_look(editor, clip);
    let mut drawing = editor
        .timeline
        .sequence()
        .media_item(clip.source)
        .and_then(|item| item.generator.clone())
        .filter(|_| sections == Sections::All)
        .map(|generator| match &editor.effects.drawing {
            Some((id, drawing)) if *id == clip.source => drawing.clone(),
            _ => generator,
        });
    let format = editor.timeline.sequence().format;
    let frame = [format.width, format.height];
    text_style_row::lend(ui.ctx(), &editor.text_styles);
    let drawing_change = drawing
        .as_mut()
        .and_then(|generator| generated::controls(ui, generator, frame));
    if graphic_layers::take_picture_request(ui) {
        editor.prompt_path(PathPurpose::GraphicPicture(clip.source));
    }
    if let Some(request) = text_style_row::take_request(ui.ctx()) {
        editor.text_style_request(request);
    }
    let video_tracks = editor.timeline.sequence().video.len();
    let keys = std::mem::take(&mut editor.effects.keys);
    let lane = match sections {
        Sections::All => lane(editor, ui, clip),
        Sections::Color => None,
    };
    let mut rows = Rows::new(time, lane, keys);
    rows.analyzing = editor.motion_analysis_progress(clip.id);
    rows.reframing = editor.reframe_analysis_progress(clip.id);
    rows.tracking = editor.mask_tracking(clip.id);
    rows.ken_burns = editor.effects.ken_burns;
    rows.simple = editor.simple();
    rows.tone = match (editor.analyzing_tone(clip.id), &editor.comparison) {
        (true, _) => ToneState::Analyzing,
        (false, Some(_)) => ToneState::ReadyToMatch,
        (false, None) => ToneState::Ready,
    };
    let mut output = match sections {
        Sections::All => look::controls(ui, &mut look, rows, video_tracks),
        Sections::Color => {
            color::controls(ui, &mut rows, &mut look.color);
            rows.output
        }
    };
    editor.effects.keys = std::mem::take(&mut output.keys);
    if sections == Sections::All {
        editor.effects.cropping = output.cropping;
    }
    if output.change.is_some() {
        let timebase = editor.timeline.sequence().timebase;
        stabilizer::retune(&mut look, timebase);
        reframe::retune(&mut look, timebase);
    }
    seek_to_key(editor, clip, output.seek);
    if let Some(on) = output.ken_burns {
        editor.set_ken_burns(clip, on);
    }
    if output.swap_ken_burns && ken_burns::swap(editor, clip, &mut look) {
        output.change = Some(Change::Commit);
    }
    if let Some(slot) = output.choose_lut {
        editor.prompt_path(PathPurpose::Lut(clip.id, slot));
    }
    if let Some(effect) = output.choose_matte {
        editor.prompt_path(PathPurpose::ImageMatte(clip.id, effect));
    }
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
    adjust_look(editor, edit, output.change);
    if let Some(goal) = output.analyze_tone {
        editor.analyze_tone(clip.id, goal, time);
    }
    if output.analyze_motion {
        editor.analyze_motion(clip.id);
    }
    if output.analyze_reframe {
        editor.analyze_reframe(clip.id);
    }
    if let Some((mask, direction, span)) = output.track_mask {
        editor.track_mask(clip.id, mask, direction, span);
    }
    if output.stop_tracking {
        editor.stop_mask_tracking();
    }
}

/// The look `clip` is shown with: the one being dragged, if any, else its own.
fn shown_look(editor: &Editor, clip: &Clip) -> Look {
    match &editor.effects.adjusting {
        Some((id, look)) if *id == clip.id => look.clone(),
        _ => clip.look.clone(),
    }
}

/// Shows the clip with the edit's look on the program and here, without keeping it.
pub(crate) fn preview_look(editor: &mut Editor, edit: SetClipLook) {
    adjust_look(editor, edit, Some(Change::Live));
}

/// Keeps the edit's look, ending any shown without being kept.
pub(crate) fn commit_look(editor: &mut Editor, edit: SetClipLook) {
    adjust_look(editor, edit, Some(Change::Commit));
}

fn adjust_look(editor: &mut Editor, edit: SetClipLook, change: Option<Change>) {
    match change {
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

fn audio_effect_controls(editor: &mut Editor, ui: &mut egui::Ui, clip: &Clip) {
    let time = keyframe_time(editor, clip);
    let mut effects = match &editor.effects.processing {
        Some((id, effects)) if *id == clip.id => effects.clone(),
        _ => clip.audio_effects.clone(),
    };
    let keys = std::mem::take(&mut editor.effects.keys);
    let rows = Rows::new(time, lane(editor, ui, clip), keys);
    let mut output = sound::effect_controls(ui, &mut effects, rows);
    editor.effects.keys = std::mem::take(&mut output.keys);
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

fn lane(editor: &Editor, ui: &egui::Ui, clip: &Clip) -> Option<Lane> {
    Lane::new(ui, editor.timeline.sequence(), clip)
}

/// Sets one of a clip's LUTs to the file at `path`, which playback reads when drawing.
pub(crate) fn set_lut(editor: &mut Editor, clip: ClipId, slot: LutSlot, path: PathBuf) {
    let Some((_, clip)) = editor.timeline.sequence().clip(clip) else {
        return;
    };
    let mut look = clip.look.clone();
    let lut = match slot {
        LutSlot::Input => &mut look.color.input_lut,
        LutSlot::Look => &mut look.color.look,
    };
    *lut = Some(path);
    editor.apply(SetClipLook {
        clip: clip.id,
        look,
    });
}

pub(crate) fn set_image_matte(editor: &mut Editor, clip: ClipId, effect: usize, path: PathBuf) {
    let Some((_, clip)) = editor.timeline.sequence().clip(clip) else {
        return;
    };
    let mut look = clip.look.clone();
    let Some(VideoEffect::ImageMatteKey(key)) = look.effects.get_mut(effect) else {
        return;
    };
    key.matte = Some(path);
    editor.apply(SetClipLook {
        clip: clip.id,
        look,
    });
}

pub(crate) fn add_graphic_picture(editor: &mut Editor, source: SourceId, path: PathBuf) {
    let generator = editor
        .timeline
        .sequence()
        .media_item(source)
        .and_then(|item| item.generator.as_ref())
        .and_then(|generator| graphic_layers::with_picture(generator, path));
    if let Some(generator) = generator {
        editor.apply(SetGenerator { source, generator });
    }
}

/// Shows the program as `edit` would leave it, without adding to the undo history.
fn show_live(editor: &mut Editor, edit: &impl Edit) {
    if let Ok(sequence) = preview(editor.timeline.sequence(), edit) {
        editor.show_program(sequence);
    }
}

/// Plays the program as `edit` would leave it without seeking, so a change to the
/// sound is heard while playback carries on.
fn hear_live(editor: &mut Editor, edit: &impl Edit) {
    if let Ok(sequence) = preview(editor.timeline.sequence(), edit) {
        editor.mix_program(sequence);
    }
}

fn clip_name<'a>(sequence: &'a Sequence, clip: &'a Clip) -> &'a str {
    let item = sequence.media_item(clip.source);
    clip.name
        .as_deref()
        .or(item.map(|item| item.name.as_str()))
        .unwrap_or("Missing media")
}
