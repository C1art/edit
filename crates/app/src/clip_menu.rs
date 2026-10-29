use egui::{Button, Ui};
use timeline::{
    ClipId, Edit, FrameFit, IntoSync, Label, SetClipLabel, SetTimeInterpolation, SyncMethod,
    TimeInterpolation, TrackKind,
};

use crate::editor::Editor;
use crate::keymap::Command;
use crate::timeline_view::label_color;

/// What a right-click on a timeline clip offers for the selection.
pub(crate) fn context(editor: &mut Editor, ui: &mut Ui) {
    if ui
        .add(Button::new("Cut").shortcut_text(editor.keymap.label(Command::Cut)))
        .clicked()
    {
        editor.cut();
    }
    if ui
        .add(Button::new("Copy").shortcut_text(editor.keymap.label(Command::Copy)))
        .clicked()
    {
        editor.copy();
    }
    let paste_attributes = Button::new("Paste Attributes…")
        .shortcut_text(editor.keymap.label(Command::PasteAttributes));
    if ui
        .add_enabled(editor.clipboard.is_some(), paste_attributes)
        .clicked()
    {
        editor.paste_attributes();
    }
    if ui.button("Remove Attributes…").clicked() {
        editor.remove_attributes();
    }
    ui.separator();
    if ui
        .add(Button::new("Clear").shortcut_text(editor.keymap.label(Command::Clear)))
        .clicked()
    {
        editor.lift();
    }
    if ui
        .add(Button::new("Ripple Delete").shortcut_text(editor.keymap.label(Command::RippleDelete)))
        .clicked()
    {
        editor.ripple_delete();
    }
    if let Some(clip) = editor.out_of_sync_selection() {
        ui.separator();
        for method in [SyncMethod::Move, SyncMethod::Slip] {
            let edit = IntoSync { clip, method };
            if ui.button(edit.name()).clicked() {
                editor.apply(edit);
            }
        }
    }
    ui.separator();
    items(editor, ui);
}

/// The Clip menu's commands on the selection, disabled while nothing is selected.
pub(crate) fn items(editor: &mut Editor, ui: &mut Ui) {
    let selected = !editor.view.selection.is_empty();
    ui.add_enabled_ui(selected, |ui| on_selection(editor, ui));
    ui.menu_button("Video Options", |ui| {
        // A frame hold also takes the clips under the playhead.
        if ui.button("Add Frame Hold").clicked() {
            editor.add_frame_hold();
        }
        ui.add_enabled_ui(selected, |ui| {
            if ui.button("Set to Frame Size").clicked() {
                editor.set_to_frame_size(FrameFit::Fit);
            }
            if ui.button("Set to Fill Frame").clicked() {
                editor.set_to_frame_size(FrameFit::Fill);
            }
        });
        ui.menu_button("Time Interpolation", |ui| time_interpolation(editor, ui));
    });
    crate::multicam::camera_menu(editor, ui);
    let scenes = Button::new("Scene Edit Detection");
    if ui
        .add_enabled(selected, scenes)
        .on_hover_text("Cut the selected clips where their shots change")
        .clicked()
    {
        editor.detect_scene_edits();
    }
    ui.separator();
    if ui.add_enabled(selected, Button::new("Nest")).clicked() {
        editor.nest_selection();
    }
    let render = Button::new("Render and Replace");
    if ui
        .add_enabled(editor.can_render_and_replace(), render)
        .on_hover_text("Render the selected nests and graphics to files the clips then show")
        .clicked()
    {
        editor.render_and_replace();
    }
    let restore = Button::new("Restore Unrendered");
    if ui
        .add_enabled(editor.can_restore_unrendered(), restore)
        .clicked()
    {
        editor.restore_unrendered();
    }
}

fn on_selection(editor: &mut Editor, ui: &mut Ui) {
    ui.menu_button("Replace With Clip", |ui| {
        let has_source = editor.source.is_some();
        if ui
            .add_enabled(has_source, Button::new("From Source Monitor"))
            .clicked()
        {
            editor.replace_with_source(false);
        }
        let matched = Button::new("From Source Monitor, Match Frame");
        if ui.add_enabled(has_source, matched).clicked() {
            editor.replace_with_source(true);
        }
    });
    let speed =
        Button::new("Speed/Duration…").shortcut_text(editor.keymap.label(Command::SpeedDuration));
    if ui.add(speed).clicked() {
        editor.speed_duration();
    }
    if ui.button("Rename…").clicked() {
        editor.rename_clips();
    }
    let enable = Button::new("Enable").shortcut_text(editor.keymap.label(Command::ToggleEnabled));
    if ui.add(enable).clicked() {
        editor.toggle_enabled();
    }
    let link = if editor.selection_linked() == Some(true) {
        "Unlink"
    } else {
        "Link"
    };
    let link = Button::new(link).shortcut_text(editor.keymap.label(Command::ToggleLink));
    if ui.add(link).clicked() {
        editor.toggle_link();
    }
    for (command, grouped) in [(Command::Group, true), (Command::Ungroup, false)] {
        let button = Button::new(command.name()).shortcut_text(editor.keymap.label(command));
        if ui.add(button).clicked() {
            editor.set_grouped(grouped);
        }
    }
    let join = Button::new(Command::JoinThroughEdits.name())
        .shortcut_text(editor.keymap.label(Command::JoinThroughEdits));
    if ui.add(join).clicked() {
        editor.join_through_edits();
    }
    let gain = Button::new("Audio Gain…").shortcut_text(editor.keymap.label(Command::AudioGain));
    if ui.add(gain).clicked() {
        editor.prompt_clip_gain();
    }
    if ui
        .button("Auto Duck…")
        .on_hover_text("Lower the selected music wherever other tracks are heard")
        .clicked()
    {
        editor.open_auto_duck();
    }
    if ui
        .button("Remix…")
        .on_hover_text("Cut or loop the selected music to a new length")
        .clicked()
    {
        editor.open_remix();
    }
    ui.menu_button("Label", |ui| labels(editor, ui));
}

/// Radio choices for the selected video clips, checked where they all agree.
fn time_interpolation(editor: &mut Editor, ui: &mut Ui) {
    let sequence = editor.timeline.sequence();
    let clips: Vec<ClipId> = editor
        .view
        .selection
        .iter()
        .filter(|&&id| {
            sequence
                .clip(id)
                .is_some_and(|(track, _)| track.kind == TrackKind::Video)
        })
        .copied()
        .collect();
    let mut current = clips
        .iter()
        .filter_map(|&id| sequence.clip(id))
        .map(|(_, clip)| clip.interpolation);
    let shared = current
        .next()
        .filter(|first| current.all(|other| other == *first));
    for interpolation in TimeInterpolation::ALL {
        let chosen = ui
            .add_enabled(
                !clips.is_empty(),
                egui::RadioButton::new(shared == Some(interpolation), interpolation.name()),
            )
            .clicked();
        if chosen {
            editor.apply(SetTimeInterpolation {
                clips: clips.clone(),
                interpolation,
            });
            ui.close();
        }
    }
}

fn labels(editor: &mut Editor, ui: &mut Ui) {
    for label in Label::ALL {
        let clicked = ui
            .horizontal(|ui| {
                let size = egui::vec2(12.0, 12.0);
                let (swatch, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                ui.painter().rect_filled(swatch, 2.0, label_color(label));
                ui.button(label.name()).clicked()
            })
            .inner;
        if clicked {
            set_label(editor, Some(label));
        }
    }
    ui.separator();
    if ui.button("Track Color").clicked() {
        set_label(editor, None);
    }
}

fn set_label(editor: &mut Editor, label: Option<Label>) {
    if editor.view.selection.is_empty() {
        editor.notice("Select clips to label");
        return;
    }
    let clips = editor.view.selection.clone();
    editor.apply(SetClipLabel { clips, label });
}
