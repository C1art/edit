use std::path::{Path, PathBuf};

use egui::Key;

use crate::editor::Editor;
use crate::error::AppError;
use crate::project_files::{quoted_name, with_project_extension, AfterDiscard};

/// Beside the project, named for it, so a copy isn't mistaken for the original.
pub fn copy_path(project: &Path) -> PathBuf {
    let stem = project
        .file_stem()
        .map_or_else(|| "Untitled".into(), |stem| stem.to_string_lossy());
    with_project_extension(project.with_file_name(format!("{stem} copy")))
}

impl Editor {
    pub(crate) fn save_copy_to(&mut self, path: PathBuf) {
        let path = with_project_extension(path);
        if self
            .project_path
            .as_deref()
            .is_some_and(|open| crate::same_path::same_path(open, &path))
        {
            self.save_project();
            return;
        }
        let sequence = self.timeline.project().clone();
        match &mut self.store {
            Some(store) => store.save_copy(path, sequence),
            None => self.notice("Cannot save: the project worker is not running"),
        }
    }

    pub(crate) fn copy_saved(&mut self, path: &Path, result: Result<(), AppError>) {
        match result {
            Ok(()) => self.notice(format!("Saved a copy as {}", quoted_name(path))),
            Err(error) => self.notice(format!("Cannot save {}: {error}", quoted_name(path))),
        }
    }
}

pub fn revert_dialog(editor: &mut Editor, ctx: &egui::Context) {
    if !editor.confirm_revert {
        return;
    }
    let Some(path) = editor.project_path.clone() else {
        editor.confirm_revert = false;
        return;
    };
    let mut reverting = None;
    crate::dialog::window(ctx, "Revert").show(ctx, |ui| {
        ui.label(format!(
            "Go back to the last saved version of {}?",
            quoted_name(&path)
        ));
        ui.weak("Changes since then are lost, and can't be undone.");
        crate::dialog::footer(ui, |ui| {
            if ui.button("Cancel").clicked() || ui.input(|input| input.key_pressed(Key::Escape)) {
                reverting = Some(false);
            }
            if ui.add(crate::dialog::primary("Revert")).clicked() {
                reverting = Some(true);
            }
        });
    });
    let Some(reverting) = reverting else {
        return;
    };
    editor.confirm_revert = false;
    if reverting {
        editor.after_discard(AfterDiscard::Open(path));
    }
}
