//! A card of the keys a first edit needs, each shown as it's bound now, so a beginner
//! can learn the few that matter without reading the whole shortcut list.

use egui::{Button, Context, RichText, Ui};

use crate::editor::Editor;
use crate::keymap::{Command, KeymapWindow};

const GROUPS: [(&str, &[(Command, &str)]); 4] = [
    (
        "Watch",
        &[
            (Command::PlayStop, "Play or stop"),
            (Command::ShuttleLeft, "Play backwards, faster each press"),
            (Command::ShuttleRight, "Play forwards, faster each press"),
            (Command::StepBack, "One frame back"),
            (Command::StepForward, "One frame forward"),
            (Command::GoToStart, "Go to the start"),
        ],
    ),
    (
        "Cut",
        &[
            (Command::AddEdit, "Snip the clips at the playhead"),
            (
                Command::RippleTrimPrevious,
                "Trim the clip's start to the playhead",
            ),
            (
                Command::RippleTrimNext,
                "Trim the clip's end to the playhead",
            ),
            (Command::Clear, "Delete, leaving a gap"),
            (Command::RippleDelete, "Delete and close the gap"),
            (Command::Undo, "Undo"),
        ],
    ),
    (
        "Tools",
        &[
            (Command::SelectionTool, "Select and move clips"),
            (Command::TypeTool, "Type a title on the picture"),
        ],
    ),
    (
        "See",
        &[
            (Command::ZoomIn, "Zoom the timeline in"),
            (Command::ZoomOut, "Zoom the timeline out"),
            (Command::ZoomToFit, "Fit the whole sequence"),
            (Command::FullScreen, "Full screen"),
        ],
    ),
];

#[derive(Default)]
pub(crate) struct EssentialKeys {
    pub(crate) open: bool,
}

/// The Help menu.
pub(crate) fn help_menu(editor: &mut Editor, ui: &mut Ui) {
    crate::theme::menu(ui, "Help", |ui| {
        if ui.button("Essential Shortcuts").clicked() {
            editor.essential_keys.open = true;
        }
        let all = Button::new("Keyboard Shortcuts…")
            .shortcut_text(editor.keymap.label(Command::KeyboardShortcuts));
        if ui.add(all).clicked() {
            editor.keymap_window = Some(KeymapWindow::default());
        }
    });
}

pub(crate) fn window(editor: &mut Editor, ctx: &Context) {
    let mut open = editor.essential_keys.open;
    let mut customize = false;
    crate::dialog::window(ctx, "Essential Shortcuts")
        .open(&mut open)
        .show(ctx, |ui| {
            ui.columns(2, |columns| {
                for (index, (heading, keys)) in GROUPS.iter().enumerate() {
                    let ui = &mut columns[index % 2];
                    ui.label(RichText::new(*heading).strong());
                    egui::Grid::new(heading).num_columns(2).show(ui, |ui| {
                        for &(command, what) in *keys {
                            let key = editor.keymap.label(command);
                            ui.label(RichText::new(key).monospace().strong());
                            ui.label(what);
                            ui.end_row();
                        }
                    });
                    ui.add_space(8.0);
                }
            });
            crate::dialog::footer(ui, |ui| {
                customize = ui.button("Change Shortcuts…").clicked();
            });
        });
    if customize {
        editor.keymap_window = Some(KeymapWindow::default());
    }
    editor.essential_keys.open = open && !customize;
}
