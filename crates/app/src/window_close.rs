use egui::{Key, ViewportCommand};

use crate::editor::Editor;
use crate::project_files::AfterDiscard;

impl Editor {
    /// Asks before closing the window over unsaved changes, and keeps the title current.
    pub(crate) fn update_window(&mut self) {
        let ctx = self.ctx.clone();
        let mut closing = ctx.input(|input| input.viewport().close_requested());
        if closing && self.exports_hold_close() {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            closing = false;
        }
        let closes = closing && (self.quitting || !self.is_dirty());
        if closing && !closes {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.confirm_discard = Some(AfterDiscard::Quit);
        }
        if closes {
            self.updates.window_closing();
        }
        self.track_window(&ctx, closes);
        self.tend_session_marker(closes);
        let name = self
            .project_path
            .as_ref()
            .and_then(|path| path.file_stem())
            .map_or_else(|| "Untitled".into(), |stem| stem.to_string_lossy());
        let mark = if self.is_dirty() { "*" } else { "" };
        let title = format!("{name}{mark} - Cledit");
        if self.window_title != title {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.window_title = title;
        }
    }
}

pub fn discard_dialog(editor: &mut Editor, ctx: &egui::Context) {
    let Some(action) = editor.confirm_discard.clone() else {
        return;
    };
    let name = editor
        .project_path
        .as_ref()
        .and_then(|path| path.file_name())
        .map_or_else(|| "Untitled".into(), |name| name.to_string_lossy());
    let mut choice = None;
    crate::dialog::window(ctx, "Unsaved Changes").show(ctx, |ui| {
        ui.label(format!("Save changes to {name}?"));
        crate::dialog::footer(ui, |ui| {
            if ui.button("Cancel").clicked() || ui.input(|input| input.key_pressed(Key::Escape)) {
                choice = Some(Choice::Cancel);
            }
            if ui.button("Don't Save").clicked() {
                choice = Some(Choice::Discard);
            }
            if ui.add(crate::dialog::primary("Save")).clicked()
                || ui.input(|input| input.key_pressed(Key::Enter))
            {
                choice = Some(Choice::Save);
            }
        });
    });
    let Some(choice) = choice else {
        return;
    };
    editor.confirm_discard = None;
    match choice {
        Choice::Save => {
            editor.after_save = Some(action);
            editor.save_project();
        }
        Choice::Discard => editor.after_discard(action),
        Choice::Cancel if action == AfterDiscard::UseOpened => editor.drop_parked_open(),
        // A later quit asks about the exports again.
        Choice::Cancel => {
            editor.exports_on_quit = Default::default();
            editor.updates.cancel_restart();
        }
    }
}

enum Choice {
    Save,
    Discard,
    Cancel,
}
