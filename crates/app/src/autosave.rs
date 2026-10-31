use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::editor::Editor;

/// How many autosaves of a project are kept before the oldest is overwritten.
pub(crate) const SLOTS: usize = 5;

/// Periodic copies of unsaved work, written beside the project in an `Auto-Save`
/// folder, or to the user's application data while the project is untitled.
pub struct Autosave {
    pub enabled: bool,
    pub minutes: u32,
    last: Instant,
    /// The timeline revision last autosaved, so an idle project isn't written again.
    saved: Option<u64>,
    slot: usize,
}

impl Default for Autosave {
    fn default() -> Self {
        Self {
            enabled: true,
            minutes: 5,
            last: Instant::now(),
            saved: None,
            slot: 0,
        }
    }
}

impl Autosave {
    /// Called when the timeline is replaced, since a new timeline restarts revisions.
    pub fn forget(&mut self) {
        self.saved = None;
        self.last = Instant::now();
    }

    fn interval(&self) -> Duration {
        Duration::from_secs(u64::from(self.minutes.max(1)) * 60)
    }
}

impl Editor {
    pub(crate) fn autosave_tick(&mut self) {
        if !self.autosave.enabled {
            return;
        }
        let due = self.autosave.last + self.autosave.interval();
        let now = Instant::now();
        if now < due {
            self.ctx.request_repaint_after(due - now);
            return;
        }
        self.autosave.last = now;
        let revision = self.timeline.revision();
        if !self.is_dirty() || self.autosave.saved == Some(revision) {
            return;
        }
        let Some(store) = &mut self.store else {
            return;
        };
        let path = autosave_path(
            self.project_path.as_deref(),
            self.autosave.slot,
            std::process::id(),
        );
        store.autosave(path, self.timeline.project().clone());
        self.autosave.saved = Some(revision);
        self.autosave.slot = (self.autosave.slot + 1) % SLOTS;
    }
}

/// Where autosave `slot` of `project` goes. Untitled work is told apart by the `session`
/// (process id) that made it, so editors open at once don't overwrite each other's.
pub(crate) fn autosave_path(project: Option<&Path>, slot: usize, session: u32) -> PathBuf {
    let number = slot + 1;
    let extension = project::EXTENSION;
    match project.and_then(|path| Some((path.parent()?, path.file_stem()?))) {
        Some((folder, stem)) => folder
            .join("Auto-Save")
            .join(format!("{}-{number}.{extension}", stem.to_string_lossy())),
        None => crate::project_store::app_folder()
            .join("Auto-Save")
            .join(format!("Untitled {session}-{number}.{extension}")),
    }
}
