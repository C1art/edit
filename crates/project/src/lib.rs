mod error;
mod json;
mod presets;
mod replace;

use std::path::Path;

use serde::{Deserialize, Serialize};
use timeline::Sequence;

pub use error::ProjectError;
use json::Value;
pub use presets::PresetsFile;
pub use replace::{rename_into_place, replace_file, retry_while_in_use};

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
        let version = format_version(&document, FORMAT_VERSION)?;
        migrate(&mut document, version)?;
        let project: Self = json::from_value(document)?;
        project
            .sequence
            .validate()
            .map_err(|error| ProjectError::Invalid(error.to_string()))?;
        Ok(project)
    }

    pub fn save(&self, path: &Path) -> Result<(), ProjectError> {
        Ok(replace_file(path, self.to_text()?.as_bytes())?)
    }

    pub fn load(path: &Path) -> Result<Self, ProjectError> {
        Self::from_text(&std::fs::read_to_string(path)?)
    }
}

/// A digest of `value` as a project saves it, the same from run to run and build to
/// build, so files named by it can be found again.
pub fn fingerprint<T: Serialize>(value: &T) -> Result<u64, ProjectError> {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    let text = json::to_value(value)?.to_text();
    Ok(text.bytes().fold(FNV_OFFSET, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME)
    }))
}

/// The format version `document` was written in, if this build reads it.
fn format_version(document: &Value, newest: u32) -> Result<u32, ProjectError> {
    let version = match document.get(VERSION_FIELD) {
        Some(Value::Integer(version)) => {
            u32::try_from(*version).map_err(|_| ProjectError::MissingVersion)?
        }
        _ => return Err(ProjectError::MissingVersion),
    };
    if version > newest {
        return Err(ProjectError::UnsupportedVersion(version));
    }
    Ok(version)
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
