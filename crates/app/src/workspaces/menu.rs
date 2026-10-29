use egui::{Button, Key};

use super::Workspaces;
use crate::editor::Editor;
use crate::effects::SideTab;
use crate::keymap::{Command, WORKSPACE_SLOTS};
use crate::panels::MediaTab;
use crate::theme;

/// The Save as New Workspace dialog.
pub(crate) struct NamingWorkspace {
    name: String,
    opened: bool,
}

pub(crate) fn window_menu(editor: &mut Editor, ui: &mut egui::Ui) {
    crate::theme::menu(ui, "Window", |ui| {
        ui.menu_button("Workspaces", |ui| workspaces_menu(editor, ui));
        ui.separator();
        for tab in MediaTab::ALL {
            let shown = editor.media_tab == tab;
            if ui.selectable_label(shown, tab.words().1).clicked() {
                editor.media_tab = tab;
            }
        }
        ui.separator();
        for tab in SideTab::ALL {
            let shown = editor.side_tab == tab;
            if ui.selectable_label(shown, tab.words().1).clicked() {
                editor.side_tab = tab;
            }
        }
        ui.separator();
        let messages = &mut editor.notifications.history_open;
        if ui.selectable_label(*messages, "Messages").clicked() {
            *messages = !*messages;
        }
    });
}

fn workspaces_menu(editor: &mut Editor, ui: &mut egui::Ui) {
    let active = editor.workspaces.active();
    let mut chosen = None;
    for (index, workspace) in editor.workspaces.list().iter().enumerate() {
        let mut button = Button::selectable(index == active, &workspace.name);
        let slot = u8::try_from(index)
            .ok()
            .filter(|&slot| slot < WORKSPACE_SLOTS);
        if let Some(slot) = slot {
            button = button.shortcut_text(editor.keymap.label(Command::Workspace(slot)));
        }
        if ui.add(button).clicked() {
            chosen = Some(index);
        }
    }
    if let Some(index) = chosen {
        editor.switch_workspace(index);
    }
    ui.separator();
    let reset = Button::new("Reset to Saved Layout")
        .shortcut_text(editor.keymap.label(Command::ResetWorkspace));
    if ui.add(reset).clicked() {
        editor.reset_workspace();
    }
    if ui.button("Save Changes to this Workspace").clicked() {
        editor.save_workspace_changes();
    }
    if ui.button("Save as New Workspace…").clicked() {
        editor.naming_workspace = Some(NamingWorkspace {
            name: format!("Workspace {}", editor.workspaces.list().len() + 1),
            opened: true,
        });
    }
    let custom = editor.workspaces.list().len() > super::BUILT_IN.len();
    ui.add_enabled_ui(custom, |ui| {
        ui.menu_button("Delete Workspace", |ui| {
            let mut deleted = None;
            let list = editor.workspaces.list().iter().enumerate();
            for (index, workspace) in list.filter(|&(index, _)| !Workspaces::is_built_in(index)) {
                if ui.button(&workspace.name).clicked() {
                    deleted = Some(index);
                }
            }
            if let Some(index) = deleted {
                editor.delete_workspace(index);
            }
        });
    });
}

/// The workspace names along the menu bar, the active one highlighted.
pub(crate) fn workspace_bar(editor: &mut Editor, ui: &mut egui::Ui) {
    let active = editor.workspaces.active();
    let mut chosen = None;
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 16.0;
        let list = editor.workspaces.list().iter().enumerate().rev();
        for (index, workspace) in list {
            if theme::tab(ui, index == active, &workspace.name).clicked() {
                chosen = Some(index);
            }
        }
    });
    if let Some(index) = chosen {
        editor.switch_workspace(index);
    }
}

pub(crate) fn dialog(editor: &mut Editor, ctx: &egui::Context) {
    let Some(naming) = &mut editor.naming_workspace else {
        return;
    };
    let mut done = None;
    crate::dialog::window(ctx, "New Workspace").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label("Name");
            let field = ui.text_edit_singleline(&mut naming.name);
            if crate::dialog::opening(ui, &mut naming.opened) {
                crate::dialog::focus_text(ui, &field, &naming.name);
            }
        });
        crate::dialog::footer(ui, |ui| {
            if ui.button("Cancel").clicked() || ui.input(|input| input.key_pressed(Key::Escape)) {
                done = Some(false);
            }
            if ui.add(crate::dialog::primary("OK")).clicked()
                || ui.input(|input| input.key_pressed(Key::Enter))
            {
                done = Some(true);
            }
        });
    });
    let Some(keep) = done else {
        return;
    };
    let name = naming.name.clone();
    editor.naming_workspace = None;
    if keep {
        if let Err(message) = editor.save_new_workspace(&name) {
            editor.notice(message);
        }
    }
}
