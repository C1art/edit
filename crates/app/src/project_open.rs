//! A project read on the project worker, put in place once it has loaded. The timeline
//! stays editable meanwhile, so edits made while a slow file loads aren't dropped
//! without asking.

use std::path::PathBuf;

use timeline::Sequence;

use crate::editor::Editor;
use crate::project_files::AfterDiscard;

#[derive(Default)]
pub(crate) struct Opening {
    /// The project generation and timeline revision when the open was asked for.
    asked: Option<(u64, u64)>,
    /// Loaded while there were edits since, until the user says what to do with them.
    parked: Option<(PathBuf, Sequence)>,
}

impl Editor {
    pub(crate) fn open_project(&mut self, path: PathBuf) {
        let Some(store) = &mut self.store else {
            self.notice("Cannot open: the project worker is not running");
            return;
        };
        store.open(path);
        self.opening.asked = Some((self.project_generation, self.timeline.revision()));
    }

    pub(crate) fn opened(&mut self, path: PathBuf, sequence: Sequence) {
        let now = (self.project_generation, self.timeline.revision());
        let edited = self.opening.asked.take() != Some(now);
        if edited && self.is_dirty() {
            self.opening.parked = Some((path, sequence));
            self.confirm_discard = Some(AfterDiscard::UseOpened);
        } else {
            self.use_opened(path, sequence);
        }
    }

    pub(crate) fn use_parked_open(&mut self) {
        if let Some((path, sequence)) = self.opening.parked.take() {
            self.use_opened(path, sequence);
        }
    }

    pub(crate) fn drop_parked_open(&mut self) {
        self.opening.parked = None;
    }

    fn use_opened(&mut self, path: PathBuf, sequence: Sequence) {
        match self.take_recovered(&path) {
            Some(project) => {
                self.replace_sequence(sequence, project);
                // Not on disk as the project yet, and no revision of the new timeline
                // will ever reach this one.
                self.saved_revision = u64::MAX;
            }
            None => {
                self.remember_project(&path);
                self.replace_sequence(sequence, Some(path));
            }
        }
    }
}
