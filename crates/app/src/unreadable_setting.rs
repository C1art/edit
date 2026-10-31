use std::path::PathBuf;

use crate::editor::Editor;
use crate::error::AppError;
use crate::project_store::Setting;
use project::ProjectError;

impl Editor {
    /// Keeps the next save from replacing everything in a settings file that couldn't
    /// be read with only what was made since: a damaged file is moved out of the way,
    /// one that may be fine for another build stays put and isn't saved over.
    pub(crate) fn unreadable_setting(&mut self, setting: Setting, error: ProjectError) {
        self.notice(format!("Cannot read {}: {error}", setting.noun()));
        let Some(store) = &mut self.store else {
            return;
        };
        match error {
            ProjectError::Syntax { .. }
            | ProjectError::Invalid(_)
            | ProjectError::MissingVersion => {
                store.set_aside_setting(setting);
            }
            ProjectError::UnsupportedVersion(_) | ProjectError::Io(_) => {
                store.hold_setting(setting);
            }
        }
    }

    pub(crate) fn setting_set_aside(
        &mut self,
        setting: Setting,
        result: Result<PathBuf, AppError>,
    ) {
        match result {
            Ok(path) => self.notice(format!(
                "Kept the unreadable {} as {}",
                setting.noun(),
                path.display()
            )),
            Err(error) => self.notice(format!(
                "Cannot move the unreadable {} aside: {error}",
                setting.noun()
            )),
        }
    }
}
