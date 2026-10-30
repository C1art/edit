//! Shows where a clip's media comes from: its file in the system's file manager, or its
//! item in the project panel.

use std::path::Path;
use std::process::Command;

use timeline::SourceId;

use crate::editor::Editor;

impl Editor {
    pub(crate) fn reveal_in_explorer(&mut self, source: SourceId) {
        let sequence = self.timeline.sequence();
        let Some(item) = sequence
            .media
            .iter()
            .find(|item| item.id == source && item.is_file())
        else {
            return;
        };
        let mut file_manager = file_manager(&item.path);
        // Starting a process can stall on a busy system, so it happens off the UI thread.
        let spawned = std::thread::Builder::new()
            .name("reveal".to_owned())
            .spawn(move || {
                if let Ok(mut shown) = file_manager.spawn() {
                    let _ = shown.wait();
                }
            });
        if let Err(error) = spawned {
            self.notice(format!("Cannot open the file's folder: {error}"));
        }
    }

    pub(crate) fn reveal_in_project(&mut self, source: SourceId) {
        self.pool.reveal(source);
    }

    /// The one selected clip's media, when there is exactly one.
    pub(crate) fn selected_source(&self) -> Option<SourceId> {
        let [clip] = self.view.selection[..] else {
            return None;
        };
        let (_, clip) = self.timeline.sequence().clip(clip)?;
        Some(clip.source)
    }
}

#[cfg(windows)]
fn file_manager(path: &Path) -> Command {
    use std::os::windows::process::CommandExt;
    let mut command = Command::new("explorer");
    // Explorer parses its own arguments and wants only the path quoted.
    command.raw_arg(format!("/select,\"{}\"", path.display()));
    command
}

#[cfg(not(windows))]
fn file_manager(path: &Path) -> Command {
    let mut command = Command::new("xdg-open");
    command.arg(path.parent().unwrap_or(path));
    command
}
