mod auto_match;
mod controls;

use egui::{Button, ComboBox, RichText, Slider};
use timeline::{
    preview, AudioType, ClipId, ClipSound, Edit, Gain, SetClipGain, SetClipsSound, TrackKind,
};

use crate::editor::Editor;
use crate::mixer::{response_state, Change};
use controls::Control;

pub(crate) use auto_match::poll;

const REPAIR: [Control; 4] = [
    Control::ReduceNoise,
    Control::ReduceRumble,
    Control::DeHum,
    Control::DeEss,
];
const CLARITY: [Control; 2] = [Control::Dynamics, Control::Equalize];

/// The sound or level being dragged, heard live and committed on release, and the clips
/// being measured to match their loudness.
#[derive(Default)]
pub(crate) struct EssentialSound {
    adjusting: Option<Vec<ClipSound>>,
    leveling: Option<Gain>,
    matching: Option<auto_match::Matching>,
}

/// Premiere's Essential Sound panel: selected audio clips are tagged as dialogue, music,
/// sound effects or ambience, and each type offers the few controls its sound needs,
/// every one standing for an effect in the clips' own chains.
pub(crate) fn ui(editor: &mut Editor, ui: &mut egui::Ui) {
    let mut sounds = selected(editor);
    let Some(first) = sounds.first() else {
        ui.label(
            "Select audio clips to tag them as dialogue, music, sound effects or ambience \
             and adjust them.",
        );
        return;
    };
    let first_type = first.audio_type;
    let shared = sounds.iter().all(|sound| sound.audio_type == first_type);
    let mut change = None;
    let Some(audio_type) = first_type.filter(|_| shared) else {
        ui.label(if shared {
            "Assign an audio type to the selected clips:"
        } else {
            "The selected clips have different types. Assign one to all of them:"
        });
        ui.horizontal_wrapped(|ui| {
            for audio_type in AudioType::ALL {
                let button = Button::new(RichText::new(audio_type.name()).heading())
                    .min_size(egui::vec2(110.0, 34.0));
                if ui.add(button).clicked() {
                    sounds
                        .iter_mut()
                        .for_each(|sound| sound.audio_type = Some(audio_type));
                    change = Some(Change::Commit);
                }
            }
        });
        apply(editor, sounds, change);
        return;
    };
    ui.horizontal(|ui| {
        ui.heading(audio_type.name());
        ui.weak(format!("{} clip(s)", sounds.len()));
        if ui.small_button("Clear Audio Type").clicked() {
            sounds.iter_mut().for_each(|sound| sound.audio_type = None);
            change = Some(Change::Commit);
        }
    });
    ui.separator();
    let clips: Vec<ClipId> = sounds.iter().map(|sound| sound.clip).collect();
    egui::ScrollArea::vertical().show(ui, |ui| {
        auto_match::section(editor, ui, &clips, audio_type);
        let mut group = |ui: &mut egui::Ui, title: &str, controls: &[Control]| {
            egui::CollapsingHeader::new(title)
                .default_open(true)
                .show(ui, |ui| {
                    for &control in controls {
                        change = change.max(control_row(ui, control, &mut sounds));
                    }
                });
        };
        match audio_type {
            AudioType::Dialogue => {
                group(ui, "Repair", &REPAIR);
                group(ui, "Clarity", &CLARITY);
                group(ui, "Creative", &[Control::Reverb]);
            }
            AudioType::Music => {}
            AudioType::Effects => {
                group(ui, "Creative", &[Control::Reverb]);
                group(ui, "Pan", &[Control::Pan]);
            }
            AudioType::Ambience => group(ui, "Creative", &[Control::Reverb]),
        }
        if matches!(audio_type, AudioType::Music | AudioType::Ambience) {
            ducking(editor, ui);
        }
        clip_volume(editor, ui, &clips);
    });
    apply(editor, sounds, change);
}

/// The selected audio clips and those linked to them, as a drag in progress has them.
fn selected(editor: &Editor) -> Vec<ClipSound> {
    let sequence = editor.timeline.sequence();
    let sounds: Vec<ClipSound> = sequence
        .with_links(&editor.view.selection)
        .into_iter()
        .filter_map(|id| sequence.clip(id))
        .filter(|(track, _)| track.kind == TrackKind::Audio)
        .map(|(_, clip)| ClipSound {
            clip: clip.id,
            audio_type: clip.audio_type,
            effects: clip.audio_effects.clone(),
        })
        .collect();
    match &editor.essential_sound.adjusting {
        Some(adjusting)
            if adjusting
                .iter()
                .map(|sound| sound.clip)
                .eq(sounds.iter().map(|sound| sound.clip)) =>
        {
            adjusting.clone()
        }
        _ => sounds,
    }
}

/// A switch for `control` and, while it is on, its preset and amount, set alike on every
/// clip in `sounds` as the first one has them.
fn control_row(ui: &mut egui::Ui, control: Control, sounds: &mut [ClipSound]) -> Option<Change> {
    let setting = sounds
        .first()
        .and_then(|sound| control.setting(&sound.effects));
    let mut on = setting.is_some();
    let switch = ui.checkbox(&mut on, control.name());
    if switch.on_hover_text(control.tip()).changed() {
        let setting = on.then(|| control.switched_on());
        for sound in sounds.iter_mut() {
            control.set(&mut sound.effects, setting);
        }
        return Some(Change::Commit);
    }
    let (mut amount, mut variant) = setting?;
    let mut change = None;
    ui.indent(control.name(), |ui| {
        let variants = control.variants();
        if let Some(chosen) = variants.get(variant) {
            ComboBox::from_id_salt(control.name())
                .selected_text(*chosen)
                .show_ui(ui, |ui| {
                    for (index, name) in variants.iter().enumerate() {
                        if ui.selectable_value(&mut variant, index, *name).clicked() {
                            change = Some(Change::Commit);
                        }
                    }
                });
        }
        let slider = ui.add(Slider::new(&mut amount, control.range()).fixed_decimals(1));
        change = change.max(response_state(&slider, None).filter(|_| interacted(&slider)));
    });
    if change.is_some() {
        for sound in sounds.iter_mut() {
            control.set(&mut sound.effects, Some((amount, variant)));
        }
    }
    change
}

/// Sliders report a change when they only clamp or round a value, so a change counts
/// only when someone is using the slider.
fn interacted(response: &egui::Response) -> bool {
    response.dragged()
        || response.drag_stopped()
        || response.clicked()
        || response.has_focus()
        || response.lost_focus()
}

fn ducking(editor: &mut Editor, ui: &mut egui::Ui) {
    egui::CollapsingHeader::new("Ducking")
        .default_open(true)
        .show(ui, |ui| {
            let button = ui
                .button("Duck Under Dialogue…")
                .on_hover_text("Lowers these clips wherever the tracks holding dialogue are heard");
            if button.clicked() {
                editor.open_auto_duck_under_dialogue();
            }
        });
}

/// The selected clips' gain, set alike on all of them.
fn clip_volume(editor: &mut Editor, ui: &mut egui::Ui, clips: &[ClipId]) {
    let sequence = editor.timeline.sequence();
    let Some((_, first)) = clips.first().and_then(|&id| sequence.clip(id)) else {
        return;
    };
    let mut db = editor.essential_sound.leveling.unwrap_or(first.gain).db();
    let mut change = None;
    egui::CollapsingHeader::new("Clip Volume")
        .default_open(true)
        .show(ui, |ui| {
            let range = Gain::SILENT.db()..=Gain::MAX.db();
            let slider = Slider::new(&mut db, range)
                .text("Level")
                .suffix(" dB")
                .fixed_decimals(1);
            let slider = ui.add(slider);
            change = response_state(&slider, None).filter(|_| interacted(&slider));
        });
    let edit = SetClipGain {
        clips: clips.to_vec(),
        gain: Gain::from_db(db),
    };
    match change {
        Some(Change::Live) => {
            hear(editor, &edit);
            editor.essential_sound.leveling = Some(edit.gain);
        }
        Some(Change::Commit) => {
            editor.essential_sound.leveling = None;
            commit(editor, edit);
        }
        None => {}
    }
}

fn apply(editor: &mut Editor, sounds: Vec<ClipSound>, change: Option<Change>) {
    let edit = SetClipsSound { clips: sounds };
    match change {
        Some(Change::Live) => {
            hear(editor, &edit);
            editor.essential_sound.adjusting = Some(edit.clips);
        }
        Some(Change::Commit) => {
            editor.essential_sound.adjusting = None;
            commit(editor, edit);
        }
        None => {}
    }
}

/// Plays the program as `edit` would leave it, without adding to the undo history.
fn hear(editor: &mut Editor, edit: &impl Edit) {
    if let Ok(sequence) = preview(editor.timeline.sequence(), edit) {
        editor.mix_program(sequence);
    }
}

/// Applies `edit`, or, when it changes nothing, goes back to the mix it was heard over.
fn commit(editor: &mut Editor, edit: impl Edit) {
    let revision = editor.timeline.revision();
    editor.apply_levels(edit);
    if editor.timeline.revision() == revision {
        editor.mix_program(editor.timeline.sequence().clone());
    }
}
