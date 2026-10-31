mod error;
mod json;
mod presets;
mod replace;
mod templates;
mod text_styles;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use timeline::Sequence;

pub use error::ProjectError;
use json::Value;
pub use presets::PresetsFile;
pub use replace::{rename_into_place, replace_file, retry_while_in_use};
pub use templates::TemplatesFile;
pub use text_styles::TextStylesFile;

/// Bump on every breaking change to the file layout and add a migration from the previous version.
pub const FORMAT_VERSION: u32 = 1;
pub const EXTENSION: &str = "cledit";
/// What projects were saved as before the rename; they still open, and save where they are.
pub const OLD_EXTENSION: &str = "editproj";

const VERSION_FIELD: &str = "format_version";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectFile {
    pub format_version: u32,
    /// The folder the project was saved in, so media moved along with it is found again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saved_in: Option<PathBuf>,
    pub sequence: Sequence,
}

/// A project as `save` writes it, borrowing the sequence rather than copying it.
#[derive(Serialize)]
struct Saving<'a> {
    format_version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    saved_in: Option<&'a Path>,
    sequence: &'a Sequence,
}

impl ProjectFile {
    pub fn new(sequence: Sequence) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            saved_in: None,
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
        let saving = Saving {
            format_version: self.format_version,
            saved_in: path.parent().filter(|folder| folder.is_absolute()),
            sequence: &self.sequence,
        };
        let text = json::to_value(&saving)?.to_text();
        Ok(replace_file(path, text.as_bytes())?)
    }

    /// Reads the project at `path`. If it was moved since it was saved, files missing from
    /// where they were are looked for where they'd be had they moved along with it.
    pub fn load(path: &Path) -> Result<Self, ProjectError> {
        let mut project = Self::from_text(&std::fs::read_to_string(path)?)?;
        if let (Some(saved_in), Some(now_in)) = (&project.saved_in, path.parent()) {
            timeline::follow_moved_project(&mut project.sequence, saved_in, now_in, Path::exists);
        }
        Ok(project)
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
    // Versions count from 1, so no build ever wrote a lower one.
    if version < 1 {
        return Err(ProjectError::Invalid(format!(
            "format version {version} doesn't exist"
        )));
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
