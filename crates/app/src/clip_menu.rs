use egui::{Button, KeyboardShortcut, Ui};
use timeline::{FrameFit, Label, SetClipLabel};

use crate::editor::Editor;
use crate::shortcuts;
use crate::timeline_view::label_color;

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
    ui.menu_button("Label", |ui| labels(editor, ui));
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
