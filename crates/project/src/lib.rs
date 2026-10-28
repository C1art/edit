mod error;
mod json;

use std::path::Path;

use serde::{Deserialize, Serialize};
use timeline::Sequence;

pub use error::ProjectError;
use json::Value;

/// Bump on every breaking change to the file layout and add a migration from the previous version.
pub const FORMAT_VERSION: u32 = 1;
pub const EXTENSION: &str = "editproj";

const VERSION_FIELD: &str = "format_version";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectFile {
    pub format_version: u32,
    pub sequence: Sequence,
}

impl ProjectFile {
    pub fn new(sequence: Sequence) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            sequence,
        }
    }

    pub fn to_text(&self) -> Result<String, ProjectError> {
        Ok(json::to_value(self)?.to_text())
    }

    /// Reads a project saved by this or any earlier version.
    pub fn from_text(text: &str) -> Result<Self, ProjectError> {
        let mut document = Value::parse(text)?;
        let version = match document.get(VERSION_FIELD) {
            Some(Value::Integer(version)) => {
                u32::try_from(*version).map_err(|_| ProjectError::MissingVersion)?
            }
            _ => return Err(ProjectError::MissingVersion),
        };
        if version > FORMAT_VERSION {
            return Err(ProjectError::UnsupportedVersion(version));
        }
        migrate(&mut document, version)?;
        json::from_value(document)
    }

    /// Writes beside `path` first, so a failed save never leaves a half-written project.
    pub fn save(&self, path: &Path) -> Result<(), ProjectError> {
        let text = self.to_text()?;
        let mut partial = path.as_os_str().to_owned();
        partial.push(".saving");
        std::fs::write(&partial, text)?;
        std::fs::rename(&partial, path).inspect_err(|_| {
            let _ = std::fs::remove_file(&partial);
        })?;
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self, ProjectError> {
        Self::from_text(&std::fs::read_to_string(path)?)
    }
}

/// Brings a document from `version` up to the current layout, one version at a time.
fn migrate(document: &mut Value, version: u32) -> Result<(), ProjectError> {
    // Version 1 is the first layout; migrations from it are added here as it changes.
    if version < 1 {
        return Err(ProjectError::UnsupportedVersion(version));
    }
    if let Some(field) = document.get_mut(VERSION_FIELD) {
        *field = Value::Integer(FORMAT_VERSION.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
