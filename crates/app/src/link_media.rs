use std::collections::HashSet;

use egui::{Label, RichText};
use timeline::SourceId;

use crate::editor::Editor;
use crate::project_files::PathPurpose;

const PATH_WIDTH: f32 = 420.0;

impl Editor {
    /// Keeps the offline set current and relinks media found beside a file just located.
    pub(crate) fn poll_offline(&mut self) {
        let pool = self.timeline.project();
        let mut shown: HashSet<SourceId> = pool
            .media
            .iter()
            .filter(|item| item.offline)
            .map(|item| item.id)
            .collect();
        let found = match &mut self.offline {
            Some(offline) => {
                let found = offline.poll(pool);
                shown.extend(offline.missing());
                found
            }
            None => Vec::new(),
        };
        if self.clip_art.offline != shown {
            self.clip_art.offline = shown;
        }
        for (id, path) in found {
            self.import(path, Some(id));
        }
    }

    /// After `source` was relinked from `old_folder` into `folder`, looks there for the
    /// other missing files.
    pub(crate) fn relinked(
        &mut self,
        source: SourceId,
        folder: Option<std::path::PathBuf>,
        old_folder: Option<std::path::PathBuf>,
    ) {
        let Some(offline) = &mut self.offline else {
            return;
        };
        if let Some(folder) = folder.filter(|_| offline.is_offline(source)) {
            offline.find_in(
                folder,
                old_folder.as_deref(),
                self.timeline.project(),
                source,
            );
        }
        offline.relinked(source);
    }
}

/// Lists missing media, as Premiere's Link Media dialog does when a project opens: each
/// can be located, which also finds the others kept beside it, or left offline.
pub fn dialog(editor: &mut Editor, ctx: &egui::Context) {
    if editor.path_prompt.is_some() {
        return;
    }
    let Some(offline) = &editor.offline else {
        return;
    };
    let project = editor.timeline.project();
    let missing: Vec<SourceId> = offline.unresolved(project).collect();
    if missing.is_empty() {
        return;
    }
    let mut locate = None;
    let mut leave = Vec::new();
    crate::dialog::window(ctx, "Link Media")
        .resizable(true)
        .default_width(560.0)
        .show(ctx, |ui| {
            ui.label("These files can't be found. Locate one and any others in the same folder are linked with it.");
            ui.separator();
            egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                egui::Grid::new("link_media")
                    .num_columns(2)
                    .striped(true)
                    .show(ui, |ui| {
                        for &id in &missing {
                            let Some(item) = project.media_item(id) else {
                                continue;
                            };
                            ui.vertical(|ui| {
                                ui.set_max_width(PATH_WIDTH);
                                ui.label(RichText::new(&item.name).strong());
                                let path = item.path.display().to_string();
                                ui.add(Label::new(RichText::new(path).small().weak()).wrap());
                            });
                            ui.horizontal(|ui| {
                                if ui.button("Locate…").clicked() {
                                    locate = Some(id);
                                }
                                if ui.button("Offline").clicked() {
                                    leave.push(id);
                                }
                            });
                            ui.end_row();
                        }
                    });
            });
            crate::dialog::footer(ui, |ui| {
                if ui
                    .button("Offline All")
                    .on_hover_text("Keep working; missing clips show as Media Offline")
                    .clicked()
                {
                    leave.extend(&missing);
                }
            });
        });
    if let Some(offline) = &mut editor.offline {
        offline.left_offline.extend(leave);
    }
    if let Some(id) = locate {
        editor.prompt_path(PathPurpose::Replace(id));
    }
}
