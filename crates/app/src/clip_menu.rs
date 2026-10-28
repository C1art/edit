use egui::{Button, KeyboardShortcut, Ui};
use timeline::FrameFit;

use crate::editor::Editor;
use crate::shortcuts;

fn shortcut(ui: &Ui, shortcut: KeyboardShortcut) -> String {
    ui.ctx().format_shortcut(&shortcut)
}

/// What a right-click on a timeline clip offers for the selection.
pub(crate) fn context(editor: &mut Editor, ui: &mut Ui) {
    if ui
        .add(Button::new("Cut").shortcut_text(shortcut(ui, shortcuts::CUT)))
        .clicked()
    {
        editor.cut();
    }
    if ui
        .add(Button::new("Copy").shortcut_text(shortcut(ui, shortcuts::COPY)))
        .clicked()
    {
        editor.copy();
    }
    let paste_attributes =
        Button::new("Paste Attributes…").shortcut_text(shortcut(ui, shortcuts::PASTE_ATTRIBUTES));
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
        .add(Button::new("Clear").shortcut_text("Delete"))
        .clicked()
    {
        editor.lift();
    }
    if ui
        .add(Button::new("Ripple Delete").shortcut_text("Shift+Delete"))
        .clicked()
    {
        editor.ripple_delete();
    }
    ui.separator();
    items(editor, ui);
}

/// The Clip menu's commands on the selection.
pub(crate) fn items(editor: &mut Editor, ui: &mut Ui) {
    let speed =
        Button::new("Speed/Duration…").shortcut_text(shortcut(ui, shortcuts::SPEED_DURATION));
    if ui.add(speed).clicked() {
        editor.speed_duration();
    }
    let enable = Button::new("Enable").shortcut_text(shortcut(ui, shortcuts::TOGGLE_ENABLED));
    if ui.add(enable).clicked() {
        editor.toggle_enabled();
    }
    let link = if editor.selection_linked() == Some(true) {
        "Unlink"
    } else {
        "Link"
    };
    let link = Button::new(link).shortcut_text(shortcut(ui, shortcuts::TOGGLE_LINK));
    if ui.add(link).clicked() {
        editor.toggle_link();
    }
    let gain = Button::new("Audio Gain…").shortcut_text("G");
    if ui.add(gain).clicked() {
        editor.prompt_clip_gain();
    }
    ui.menu_button("Video Options", |ui| {
        if ui.button("Add Frame Hold").clicked() {
            editor.add_frame_hold();
        }
        if ui.button("Set to Frame Size").clicked() {
            editor.set_to_frame_size(FrameFit::Fit);
        }
        if ui.button("Set to Fill Frame").clicked() {
            editor.set_to_frame_size(FrameFit::Fill);
        }
    });
    ui.separator();
    if ui.button("Nest").clicked() {
        editor.nest_selection();
    }
}
