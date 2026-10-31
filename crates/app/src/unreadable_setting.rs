use std::path::PathBuf;

use crate::editor::Editor;
use crate::error::AppError;
use crate::project_store::Setting;

impl Editor {
    /// Moves a settings file that couldn't be read out of the way: the next save would
    /// otherwise replace everything in it with only what was made since.
    pub(crate) fn unreadable_setting(&mut self, setting: Setting, error: impl std::fmt::Display) {
        self.notice(format!("Cannot read {}: {error}", setting.noun()));
        if let Some(store) = &mut self.store {
            store.set_aside_setting(setting);
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
