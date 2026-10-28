use egui::{CollapsingHeader, Sense, TextEdit};
use timeline::{
    ApplyEffect, AudioEffect, ClipEffect, ClipId, EffectCategory, EffectPreset, Ticks, TrackKind,
    TrackRef, TransitionKind, VideoEffect,
};

use crate::editor::Editor;
use crate::effects::{add_transition, apply_transition, audio_category, AUDIO_CATEGORIES};

/// Premiere's bins for video transitions, as [`TransitionKind::category`] names them.
const TRANSITION_CATEGORIES: [&str; 7] = [
    "3D Motion",
    "Dissolve",
    "Iris",
    "Page Peel",
    "Slide",
    "Wipe",
    "Zoom",
];

/// An entry of the Effects panel, carried while it is dragged onto the timeline.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum EffectItem {
    Video(Box<VideoEffect>),
    Audio(AudioEffect),
    Transition(TrackKind, TransitionKind),
    Preset(EffectPreset),
}

impl EffectItem {
    /// The kind of track the item goes on, unless it suits both.
    pub(crate) fn track_kind(&self) -> Option<TrackKind> {
        match self {
            Self::Video(_) => Some(TrackKind::Video),
            Self::Audio(_) => Some(TrackKind::Audio),
            Self::Transition(kind, _) => Some(*kind),
            Self::Preset(_) => None,
        }
    }

    fn name(&self) -> &str {
        match self {
            Self::Preset(preset) => &preset.name,
            Self::Video(effect) => effect.name(),
            Self::Audio(effect) => effect.name(),
            Self::Transition(TrackKind::Video, kind) => kind.name(),
            Self::Transition(TrackKind::Audio, kind) => kind.audio_name(),
        }
    }
}

/// An Effects panel entry released over a track.
#[derive(Debug, Clone)]
pub struct EffectDrop {
    pub(crate) item: EffectItem,
    pub track: TrackRef,
    pub at: Ticks,
}

/// Lists every effect and transition by kind and category, filtered by `search`.
pub(crate) fn ui(editor: &mut Editor, ui: &mut egui::Ui) {
    ui.add(
        TextEdit::singleline(&mut editor.effects_search)
            .hint_text("Search effects")
            .desired_width(f32::INFINITY),
    );
    ui.label("Drag onto a clip or cut, or double-click to apply to the selection");
    let search = editor.effects_search.trim().to_lowercase();
    let searching = !search.is_empty();
    let shown = |item: &EffectItem| item.name().to_lowercase().contains(&search);
    let mut chosen = None;
    let mut preset_action = None;
    egui::ScrollArea::vertical().show(ui, |ui| {
        let presets = &editor.presets;
        preset_action = presets_folder(ui, presets, searching, &shown, &mut chosen);
        let video = VideoEffect::catalog();
        let groups = EffectCategory::ALL.map(|category| {
            let items = video
                .iter()
                .filter(|effect| effect.category() == category)
                .map(|effect| EffectItem::Video(Box::new(effect.clone())));
            (category.name(), items.collect::<Vec<_>>())
        });
        folder(ui, "Video Effects", searching, &groups, &shown, &mut chosen);
        let groups = TRANSITION_CATEGORIES.map(|category| {
            let items = TransitionKind::VIDEO
                .iter()
                .filter(|kind| kind.category() == category)
                .map(|&kind| EffectItem::Transition(TrackKind::Video, kind));
            (category, items.collect::<Vec<_>>())
        });
        folder(
            ui,
            "Video Transitions",
            searching,
            &groups,
            &shown,
            &mut chosen,
        );
        let audio = AudioEffect::catalog();
        let groups = AUDIO_CATEGORIES.map(|category| {
            let items = audio
                .iter()
                .filter(|effect| audio_category(effect) == category)
                .map(|effect| EffectItem::Audio(effect.clone()));
            (category, items.collect::<Vec<_>>())
        });
        folder(ui, "Audio Effects", searching, &groups, &shown, &mut chosen);
        let transitions = TransitionKind::AUDIO
            .iter()
            .map(|&kind| EffectItem::Transition(TrackKind::Audio, kind));
        let groups = [("", transitions.collect())];
        folder(
            ui,
            "Audio Transitions",
            searching,
            &groups,
            &shown,
            &mut chosen,
        );
    });
    if let Some(item) = chosen {
        apply_to_selection(editor, item);
    }
    match preset_action {
        Some(PresetAction::Rename(index)) => editor.rename_preset(index),
        Some(PresetAction::Delete(index)) => editor.delete_preset(index),
        None => {}
    }
}

enum PresetAction {
    Rename(usize),
    Delete(usize),
}

/// The saved presets, which can be renamed or deleted from their context menus.
fn presets_folder(
    ui: &mut egui::Ui,
    presets: &[EffectPreset],
    searching: bool,
    shown: &dyn Fn(&EffectItem) -> bool,
    chosen: &mut Option<EffectItem>,
) -> Option<PresetAction> {
    let items = presets
        .iter()
        .map(|preset| EffectItem::Preset(preset.clone()))
        .enumerate();
    let matching: Vec<_> = items.filter(|(_, item)| shown(item)).collect();
    if searching && matching.is_empty() {
        return None;
    }
    let mut action = None;
    CollapsingHeader::new("Presets")
        .open(searching.then_some(true))
        .show(ui, |ui| {
            if presets.is_empty() {
                ui.weak("Keep a clip's effects with Save Preset… in Effect Controls");
            }
            for (index, item) in &matching {
                let response = entry(ui, item, chosen).on_hover_text(contents(&presets[*index]));
                response.context_menu(|ui| {
                    if ui.button("Rename…").clicked() {
                        action = Some(PresetAction::Rename(*index));
                    }
                    if ui.button("Delete").clicked() {
                        action = Some(PresetAction::Delete(*index));
                    }
                });
            }
        });
    action
}

/// The effects a preset holds, one to a line.
fn contents(preset: &EffectPreset) -> String {
    let video = preset.video.iter().map(VideoEffect::name);
    let audio = preset.audio.iter().map(AudioEffect::name);
    video.chain(audio).collect::<Vec<_>>().join("\n")
}

/// One top-level folder of `groups`, each an optional subfolder name and its entries.
/// Folders open while searching so that matches show.
fn folder(
    ui: &mut egui::Ui,
    name: &str,
    searching: bool,
    groups: &[(&str, Vec<EffectItem>)],
    shown: &dyn Fn(&EffectItem) -> bool,
    chosen: &mut Option<EffectItem>,
) {
    let matching = groups.iter().map(|(group, items)| {
        let items: Vec<_> = items.iter().filter(|item| shown(item)).collect();
        (*group, items)
    });
    let matching: Vec<_> = matching.filter(|(_, items)| !items.is_empty()).collect();
    if matching.is_empty() {
        return;
    }
    CollapsingHeader::new(name)
        .open(searching.then_some(true))
        .show(ui, |ui| {
            for (group, items) in matching {
                if group.is_empty() {
                    entries(ui, &items, chosen);
                } else {
                    CollapsingHeader::new(group)
                        .open(searching.then_some(true))
                        .show(ui, |ui| entries(ui, &items, chosen));
                }
            }
        });
}

fn entries(ui: &mut egui::Ui, items: &[&EffectItem], chosen: &mut Option<EffectItem>) {
    for &item in items {
        entry(ui, item, chosen);
    }
}

/// An item that can be dragged out, or double-clicked to choose it.
fn entry(ui: &mut egui::Ui, item: &EffectItem, chosen: &mut Option<EffectItem>) -> egui::Response {
    let label = egui::Label::new(item.name())
        .selectable(false)
        .sense(Sense::click_and_drag());
    let response = ui.add(label);
    if response.drag_started() {
        response.dnd_set_drag_payload(item.clone());
    }
    if response.double_clicked() {
        *chosen = Some(item.clone());
    }
    if response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    } else if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }
    response
}

fn apply_to_selection(editor: &mut Editor, item: EffectItem) {
    let effect = match item {
        EffectItem::Video(effect) => ClipEffect::Video(effect),
        EffectItem::Audio(effect) => ClipEffect::Audio(effect),
        EffectItem::Preset(preset) => ClipEffect::Preset(preset),
        EffectItem::Transition(kind, transition) => {
            apply_transition(editor, kind, transition);
            return;
        }
    };
    if editor.view.selection.is_empty() {
        editor.notice("Select clips to apply the effect to");
        return;
    }
    let clips = editor.view.selection.clone();
    apply_effect(editor, clips, effect);
}

/// Applies an effect released over a clip to it, or to the whole selection when that
/// holds it; and a transition to the nearest cut or clip end.
pub(crate) fn drop_effect(editor: &mut Editor, drop: EffectDrop) {
    if let Some(kind) = drop
        .item
        .track_kind()
        .filter(|&kind| kind != drop.track.kind)
    {
        let kind = match kind {
            TrackKind::Video => "video",
            TrackKind::Audio => "audio",
        };
        editor.notice(format!("{} goes on {kind} clips", drop.item.name()));
        return;
    }
    let effect = match drop.item {
        EffectItem::Video(effect) => ClipEffect::Video(effect),
        EffectItem::Audio(effect) => ClipEffect::Audio(effect),
        EffectItem::Preset(preset) => ClipEffect::Preset(preset),
        EffectItem::Transition(_, kind) => {
            add_transition(editor, drop.track, drop.at, kind);
            return;
        }
    };
    let sequence = editor.timeline.sequence();
    let Some(clip) = sequence
        .track(drop.track)
        .and_then(|track| track.clip_at(drop.at))
    else {
        return;
    };
    let clips = if editor.view.selection.contains(&clip.id) {
        editor.view.selection.clone()
    } else {
        vec![clip.id]
    };
    apply_effect(editor, clips, effect);
}

fn apply_effect(editor: &mut Editor, clips: Vec<ClipId>, effect: ClipEffect) {
    if !editor.apply(ApplyEffect { clips, effect }) {
        editor.notice("None of those clips takes that effect");
    }
}
