use egui::{Key, ViewportCommand};

use crate::editor::Editor;
use crate::export::ExportDialog;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ExportsOnQuit {
    #[default]
    Ask,
    Asking,
    Stop,
}

impl Editor {
    /// Exports that closing the window would stop or never start.
    fn unfinished_exports(&self) -> usize {
        let dialog = self
            .export
            .as_ref()
            .and_then(ExportDialog::running_path)
            .is_some();
        usize::from(dialog) + self.export_queue.pending_paths().len()
    }

    /// Whether closing now must wait for the user to agree to stop the exports.
    pub(crate) fn exports_hold_close(&mut self) -> bool {
        if self.exports_on_quit == ExportsOnQuit::Stop || self.unfinished_exports() == 0 {
            return false;
        }
        self.exports_on_quit = ExportsOnQuit::Asking;
        true
    }
}

pub(crate) fn dialog(editor: &mut Editor, ctx: &egui::Context) {
    if editor.exports_on_quit != ExportsOnQuit::Asking {
        return;
    }
    let count = editor.unfinished_exports();
    if count == 0 {
        editor.exports_on_quit = ExportsOnQuit::Ask;
        return;
    }
    let mut choice = None;
    crate::dialog::window(ctx, "Exports Not Finished").show(ctx, |ui| {
        ui.label(match count {
            1 => "An export hasn't finished. Quitting stops it.".to_owned(),
            _ => format!("{count} exports haven't finished. Quitting stops them."),
        });
        crate::dialog::footer(ui, |ui| {
            if ui.button("Cancel").clicked() || ui.input(|input| input.key_pressed(Key::Escape)) {
                choice = Some(ExportsOnQuit::Ask);
            }
            if ui.add(crate::dialog::primary("Quit")).clicked() {
                choice = Some(ExportsOnQuit::Stop);
            }
        });
    });
    if let Some(choice) = choice {
        editor.exports_on_quit = choice;
        if choice == ExportsOnQuit::Ask {
            editor.updates.cancel_restart();
        }
        if choice == ExportsOnQuit::Stop {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }
}
