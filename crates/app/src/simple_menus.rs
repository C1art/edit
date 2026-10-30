//! Simple mode's menus: the commands a short edit needs, each doing just what its
//! shortcut does.

use egui::{Button, Ui};
use timeline::{FrameFit, Generator, Rgb, Ticks, TrackKind};

use crate::editor::Editor;
use crate::keymap::Command;
use crate::placing;
use crate::ui_mode::{SIMPLE_MEDIA_TABS, SIMPLE_SIDE_TABS};

pub(crate) fn menus(editor: &mut Editor, ui: &mut Ui) {
    crate::theme::menu(ui, "File", |ui| file(editor, ui));
    crate::theme::menu(ui, "Edit", |ui| edit(editor, ui));
    crate::theme::menu(ui, "Clip", |ui| {
        ui.add_enabled_ui(!editor.view.selection.is_empty(), |ui| clip(editor, ui));
    });
    crate::theme::menu(ui, "Markers", |ui| markers(editor, ui));
    crate::theme::menu(ui, "Sequence", |ui| sequence(editor, ui));
    crate::theme::menu(ui, "Window", |ui| window(editor, ui));
}

/// A menu entry for `command`, labelled `label` and run as its shortcut runs it.
fn item(editor: &mut Editor, ui: &mut Ui, command: Command, label: &str, enabled: bool) {
    let button = Button::new(label).shortcut_text(editor.keymap.label(command));
    if ui.add_enabled(enabled, button).clicked() {
        crate::shortcuts::run(editor, command);
    }
}

fn file(editor: &mut Editor, ui: &mut Ui) {
    item(editor, ui, Command::NewProject, "New Project", true);
    item(editor, ui, Command::OpenProject, "Open Project…", true);
    crate::recent_projects::menu(editor, ui);
    let dirty = editor.is_dirty();
    item(editor, ui, Command::Save, "Save", dirty);
    item(editor, ui, Command::SaveAs, "Save As…", true);
    ui.separator();
    item(editor, ui, Command::Import, "Import…", true);
    ui.menu_button("New", |ui| {
        item(editor, ui, Command::NewTitle, "Title", true);
        crate::title_templates::menu(ui, editor);
        if ui.button("Color Matte").clicked() {
            let color = Rgb::new(32, 32, 48);
            placing::new_generated(editor, Generator::ColorMatte { color });
        }
    });
    ui.separator();
    let has_program = editor.timeline.sequence().duration() > Ticks::ZERO;
    item(
        editor,
        ui,
        Command::ExportMedia,
        "Export Media…",
        has_program,
    );
    item(
        editor,
        ui,
        Command::ExportFrame,
        "Export Frame…",
        has_program,
    );
    ui.separator();
    item(editor, ui, Command::Quit, "Quit", true);
}

fn edit(editor: &mut Editor, ui: &mut Ui) {
    let undo = editor
        .timeline
        .undo_name()
        .map(|name| format!("Undo {name}"));
    let can_undo = undo.is_some();
    let undo = undo.unwrap_or_else(|| "Undo".to_owned());
    item(editor, ui, Command::Undo, &undo, can_undo);
    let redo = editor
        .timeline
        .redo_name()
        .map(|name| format!("Redo {name}"));
    let can_redo = redo.is_some();
    let redo = redo.unwrap_or_else(|| "Redo".to_owned());
    item(editor, ui, Command::Redo, &redo, can_redo);
    ui.separator();
    let selected = !editor.view.selection.is_empty();
    item(editor, ui, Command::Cut, "Cut", selected);
    item(editor, ui, Command::Copy, "Copy", selected);
    let can_paste = editor.clipboard.is_some();
    item(editor, ui, Command::Paste, "Paste", can_paste);
    ui.separator();
    let removable = selected || editor.view.gap.is_some();
    item(editor, ui, Command::Clear, "Delete", removable);
    item(
        editor,
        ui,
        Command::RippleDelete,
        "Ripple Delete",
        removable,
    );
    ui.separator();
    let has_clips = editor.timeline.sequence().has_clips();
    item(editor, ui, Command::SelectAll, "Select All", has_clips);
    item(editor, ui, Command::DeselectAll, "Deselect All", selected);
    ui.separator();
    ui.menu_button("Preferences", |ui| {
        crate::preferences_menu::menu(editor, ui)
    });
    item(
        editor,
        ui,
        Command::KeyboardShortcuts,
        "Keyboard Shortcuts…",
        true,
    );
}

/// The Clip menu, and what a right-click on a clip offers after the clipboard.
fn clip(editor: &mut Editor, ui: &mut Ui) {
    item(editor, ui, Command::SpeedDuration, "Speed/Duration…", true);
    item(editor, ui, Command::AudioGain, "Audio Gain…", true);
    let link = if editor.selection_linked() == Some(true) {
        "Unlink"
    } else {
        "Link"
    };
    item(editor, ui, Command::ToggleLink, link, true);
    let sequence = editor.timeline.sequence();
    let all_enabled = editor
        .view
        .selection
        .iter()
        .filter_map(|&id| sequence.clip(id))
        .all(|(_, clip)| !clip.disabled);
    let enable = if all_enabled { "Disable" } else { "Enable" };
    item(editor, ui, Command::ToggleEnabled, enable, true);
    if ui.button("Rename…").clicked() {
        editor.rename_clips();
    }
    ui.separator();
    let transitions = Button::new("Add Transitions")
        .shortcut_text(editor.keymap.label(Command::TransitionsToSelection));
    if ui
        .add(transitions)
        .on_hover_text("Puts the default transition on both ends of each selected clip")
        .clicked()
    {
        crate::effects::apply_default_transitions_to_selection(editor);
    }
    if ui.button("Remove Transitions").clicked() {
        crate::effects::remove_transitions_from_selection(editor);
    }
    ui.separator();
    if ui
        .button("Fill Frame")
        .on_hover_text("Scales the picture to cover the whole frame")
        .clicked()
    {
        editor.set_to_frame_size(FrameFit::Fill);
    }
    if ui
        .button("Fit to Frame")
        .on_hover_text("Scales the picture to show all of it inside the frame")
        .clicked()
    {
        editor.set_to_frame_size(FrameFit::Fit);
    }
    if ui
        .button("Freeze Frame")
        .on_hover_text("Holds the frame under the playhead for the rest of the clip")
        .clicked()
    {
        editor.add_frame_hold();
    }
}

/// What a right-click on a timeline clip offers in Simple mode.
pub(crate) fn clip_context(editor: &mut Editor, ui: &mut Ui) {
    item(editor, ui, Command::Cut, "Cut", true);
    item(editor, ui, Command::Copy, "Copy", true);
    item(editor, ui, Command::Clear, "Delete", true);
    item(editor, ui, Command::RippleDelete, "Ripple Delete", true);
    ui.separator();
    clip(editor, ui);
}

fn markers(editor: &mut Editor, ui: &mut Ui) {
    item(editor, ui, Command::AddMarker, "Add Marker", true);
    let sequence = editor.timeline.sequence();
    let playhead = editor.playhead();
    let has_next = sequence.marker_after(playhead).is_some();
    let has_previous = sequence.marker_before(playhead).is_some();
    item(
        editor,
        ui,
        Command::NextMarker,
        "Go to Next Marker",
        has_next,
    );
    item(
        editor,
        ui,
        Command::PreviousMarker,
        "Go to Previous Marker",
        has_previous,
    );
    let has_markers = editor.has_markers();
    item(
        editor,
        ui,
        Command::ClearAllMarkers,
        "Clear All Markers",
        has_markers,
    );
    if ui
        .button("Add Markers at Beats…")
        .on_hover_text("Mark the beats of the selected music")
        .clicked()
    {
        editor.open_beat_markers();
    }
    ui.separator();
    let marks = editor.timeline.sequence().marks;
    item(editor, ui, Command::MarkIn, "Mark In", true);
    item(editor, ui, Command::MarkOut, "Mark Out", true);
    item(
        editor,
        ui,
        Command::ClearInOut,
        "Clear In and Out",
        marks.is_set(),
    );
}

fn sequence(editor: &mut Editor, ui: &mut Ui) {
    if ui.button("Sequence Settings…").clicked() {
        editor.open_sequence_settings();
    }
    ui.separator();
    let has_clips = editor.timeline.sequence().has_clips();
    item(editor, ui, Command::AddEdit, "Cut at Playhead", has_clips);
    item(
        editor,
        ui,
        Command::VideoTransition,
        "Apply Video Transition",
        has_clips,
    );
    item(
        editor,
        ui,
        Command::AudioTransition,
        "Apply Audio Transition",
        has_clips,
    );
    if ui
        .add_enabled(has_clips, Button::new("Close Gaps"))
        .on_hover_text("Remove every stretch of time no track uses")
        .clicked()
    {
        editor.close_all_gaps();
    }
    ui.separator();
    if ui.button("Add Video Track").clicked() {
        editor.add_track(TrackKind::Video);
    }
    if ui.button("Add Audio Track").clicked() {
        editor.add_track(TrackKind::Audio);
    }
    if ui.button("Record Voiceover…").clicked() {
        editor.open_voiceover();
    }
}

fn window(editor: &mut Editor, ui: &mut Ui) {
    for tab in SIMPLE_MEDIA_TABS {
        if ui
            .selectable_label(editor.media_tab == tab, tab.words().1)
            .clicked()
        {
            editor.media_tab = tab;
        }
    }
    ui.separator();
    for tab in SIMPLE_SIDE_TABS {
        if ui
            .selectable_label(editor.side_tab == tab, tab.words().1)
            .clicked()
        {
            editor.side_tab = tab;
        }
    }
    ui.separator();
    item(editor, ui, Command::FullScreen, "Full Screen", true);
    let messages = &mut editor.notifications.history_open;
    if ui.selectable_label(*messages, "Messages").clicked() {
        *messages = !*messages;
    }
}
