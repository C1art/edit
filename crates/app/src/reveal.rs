//! Shows where a clip's media comes from: its file in the system's file manager or the
//! application that opens it, or its item in the project panel.

use std::path::{Path, PathBuf};
use std::process::Command;

use timeline::SourceId;

use crate::editor::Editor;

impl Editor {
    pub(crate) fn reveal_in_explorer(&mut self, source: SourceId) {
        if let Some(path) = self.file_of(source) {
            self.reveal_file(&path);
        }
    }

    pub(crate) fn reveal_file(&mut self, path: &Path) {
        self.start(file_manager(path), "Cannot open the file's folder");
    }

    /// Opens `source`'s file in the application the system opens it with, as Premiere's
    /// Edit Original does.
    pub(crate) fn edit_original(&mut self, source: SourceId) {
        if let Some(path) = self.file_of(source) {
            self.start(default_application(&path), "Cannot open the file");
        }
    }

    fn file_of(&self, source: SourceId) -> Option<PathBuf> {
        self.timeline
            .sequence()
            .media
            .iter()
            .find(|item| item.id == source && item.is_file())
            .map(|item| item.path.clone())
    }

    fn start(&mut self, mut command: Command, failure: &str) {
        // Starting a process can stall on a busy system, so it happens off the UI thread.
        let spawned = std::thread::Builder::new()
            .name("reveal".to_owned())
            .spawn(move || {
                if let Ok(mut shown) = command.spawn() {
                    let _ = shown.wait();
                }
            });
        if let Err(error) = spawned {
            self.notice(format!("{failure}: {error}"));
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

#[cfg(windows)]
fn default_application(path: &Path) -> Command {
    use std::os::windows::process::CommandExt;
    let mut command = Command::new("explorer");
    command.raw_arg(format!("\"{}\"", path.display()));
    command
}

#[cfg(not(windows))]
fn default_application(path: &Path) -> Command {
    let mut command = Command::new("xdg-open");
    command.arg(path);
    command
}
