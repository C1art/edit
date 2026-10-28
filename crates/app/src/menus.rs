use std::path::PathBuf;

use egui::{Button, Key, MenuBar, ViewportCommand};
use timeline::{PlaceMode, TrackKind};

use crate::editor::Editor;
use crate::placing;
use crate::shortcuts;
use crate::timeline_view::Tool;

pub fn menu_bar(editor: &mut Editor, ui: &mut egui::Ui) {
    MenuBar::new().ui(ui, |ui| {
        let shortcut = |ui: &egui::Ui, shortcut| ui.ctx().format_shortcut(&shortcut);
        ui.menu_button("File", |ui| {
            let import = Button::new("Import…").shortcut_text(shortcut(ui, shortcuts::IMPORT));
            if ui.add(import).clicked() {
                editor.import_prompt = Some(String::new());
            }
            if ui.button("Quit").clicked() {
                ui.ctx().send_viewport_cmd(ViewportCommand::Close);
            }
        });
        ui.menu_button("Edit", |ui| {
            let undo = editor
                .timeline
                .undo_name()
                .map_or("Undo".to_owned(), |name| format!("Undo {name}"));
            let undo = Button::new(undo).shortcut_text(shortcut(ui, shortcuts::UNDO));
            if ui
                .add_enabled(editor.timeline.undo_name().is_some(), undo)
                .clicked()
            {
                editor.undo();
            }
            let redo = editor
                .timeline
                .redo_name()
                .map_or("Redo".to_owned(), |name| format!("Redo {name}"));
            let redo = Button::new(redo).shortcut_text(shortcut(ui, shortcuts::REDO));
            if ui
                .add_enabled(editor.timeline.redo_name().is_some(), redo)
                .clicked()
            {
                editor.redo();
            }
        });
        ui.menu_button("Clip", |ui| {
            if ui.add(Button::new("Insert").shortcut_text(",")).clicked() {
                placing::edit_from_source(editor, PlaceMode::Insert);
            }
            if ui
                .add(Button::new("Overwrite").shortcut_text("."))
                .clicked()
            {
                placing::edit_from_source(editor, PlaceMode::Overwrite);
            }
        });
        ui.menu_button("Sequence", |ui| {
            let add_edit = Button::new("Add Edit").shortcut_text(shortcut(ui, shortcuts::ADD_EDIT));
            if ui.add(add_edit).clicked() {
                editor.add_edit();
            }
            if ui
                .add(Button::new("Lift").shortcut_text("Delete"))
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
            if ui.button("Add Video Track").clicked() {
                editor.add_track(TrackKind::Video);
            }
            if ui.button("Add Audio Track").clicked() {
                editor.add_track(TrackKind::Audio);
            }
        });
    });
}

pub fn timeline_toolbar(editor: &mut Editor, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        for tool in Tool::ALL {
            if ui
                .selectable_label(editor.view.tool == tool, tool.label())
                .on_hover_text(tool.hint())
                .clicked()
            {
                editor.view.tool = tool;
            }
        }
        ui.separator();
        ui.toggle_value(&mut editor.view.snapping, "Snap")
            .on_hover_text("Snap to clip edges and the playhead (S)");
        ui.separator();
        if ui.button("−").on_hover_text("Zoom out (-)").clicked() {
            editor.view.zoom(0.5, None);
        }
        if ui.button("+").on_hover_text("Zoom in (=)").clicked() {
            editor.view.zoom(2.0, None);
        }
        if ui
            .button("Fit")
            .on_hover_text("Zoom to sequence (\\)")
            .clicked()
        {
            editor.view.zoom_to_fit(editor.timeline.sequence());
        }
        ui.separator();
        if ui.button("+ Video").clicked() {
            editor.add_track(TrackKind::Video);
        }
        if ui.button("+ Audio").clicked() {
            editor.add_track(TrackKind::Audio);
        }
    });
}

pub fn import_dialog(editor: &mut Editor, ctx: &egui::Context) {
    let Some(prompt) = &mut editor.import_prompt else {
        return;
    };
    let mut submitted = None;
    let mut cancelled = false;
    egui::Window::new("Import media")
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label("Path to a video or audio file (or drop files on the window):");
            let field = ui.add(egui::TextEdit::singleline(prompt).desired_width(480.0));
            field.request_focus();
            let entered = field.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter));
            ui.horizontal(|ui| {
                if ui.button("Import").clicked() || entered {
                    submitted = Some(PathBuf::from(prompt.trim().trim_matches('"')));
                }
                if ui.button("Cancel").clicked() || ui.input(|input| input.key_pressed(Key::Escape))
                {
                    cancelled = true;
                }
            });
        });
    if let Some(path) = submitted.filter(|path| !path.as_os_str().is_empty()) {
        editor.import_prompt = None;
        editor.import(path);
    } else if cancelled {
        editor.import_prompt = None;
    }
}
