use std::path::{Path, PathBuf};

use crate::editor::Editor;
use crate::project_files::AfterDiscard;
use crate::project_store::Setting;

const KEPT: usize = 10;

/// Projects opened or saved lately, the latest first.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RecentProjects {
    paths: Vec<PathBuf>,
}

impl RecentProjects {
    pub fn from_text(text: &str) -> Self {
        let paths = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(PathBuf::from)
            .take(KEPT)
            .collect();
        Self { paths }
    }

    pub fn to_text(&self) -> String {
        self.paths
            .iter()
            .map(|path| format!("{}\n", path.display()))
            .collect()
    }

    /// Puts `path` first; whether that changed the list.
    fn remember(&mut self, path: &Path) -> bool {
        if self.paths.first().is_some_and(|first| first == path) {
            return false;
        }
        self.paths.retain(|kept| kept != path);
        self.paths.insert(0, path.to_owned());
        self.paths.truncate(KEPT);
        true
    }
}

impl Editor {
    pub(crate) fn load_recent_projects(&mut self, text: &str) {
        // A project opened before the list was read stays at its top.
        let early = std::mem::replace(&mut self.recent_projects, RecentProjects::from_text(text));
        let changed = early.paths.iter().rev().fold(false, |changed, path| {
            self.recent_projects.remember(path) | changed
        });
        if changed {
            self.save_recent_projects();
        }
    }

    pub(crate) fn remember_project(&mut self, path: &Path) {
        if self.recent_projects.remember(path) {
            self.save_recent_projects();
        }
    }

    fn save_recent_projects(&mut self) {
        if let Some(store) = &mut self.store {
            store.write_setting(Setting::RecentProjects, self.recent_projects.to_text());
        }
    }
}

/// The File menu's list of recent projects, each opening in place of this one.
pub fn menu(editor: &mut Editor, ui: &mut egui::Ui) {
    let empty = editor.recent_projects.paths.is_empty();
    ui.add_enabled_ui(!empty, |ui| {
        ui.menu_button("Open Recent", |ui| {
            let mut chosen = None;
            for path in &editor.recent_projects.paths {
                let name = path.file_stem().unwrap_or(path.as_os_str());
                let button = ui.button(name.to_string_lossy());
                if button.on_hover_text(path.display().to_string()).clicked() {
                    chosen = Some(path.clone());
                }
            }
            ui.separator();
            if ui.button("Clear Recent Projects").clicked() {
                editor.recent_projects.paths.clear();
                editor.save_recent_projects();
            }
            if let Some(path) = chosen {
                editor.discard_then(AfterDiscard::Open(path));
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_project_used_again_moves_to_the_top_and_only_ten_are_kept() {
        let mut recent = RecentProjects::default();
        for index in 0..12 {
            recent.remember(Path::new(&format!("Edits/{index}.edit")));
        }
        assert!(!recent.remember(Path::new("Edits/11.edit")));
        assert!(recent.remember(Path::new("Edits/5.edit")));
        let kept = RecentProjects::from_text(&recent.to_text());
        let names: Vec<_> = kept
            .paths
            .iter()
            .filter_map(|path| path.file_stem())
            .collect();
        assert_eq!(
            names,
            ["5", "11", "10", "9", "8", "7", "6", "4", "3", "2"].map(std::ffi::OsStr::new)
        );
    }
}
