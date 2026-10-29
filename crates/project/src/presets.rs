use serde::{Deserialize, Serialize};
use timeline::EffectPreset;

use crate::json::{self, Value};
use crate::{format_version, ProjectError};

/// Bump on every breaking change to the presets layout, migrating older files.
const PRESETS_VERSION: u32 = 1;

/// The effect presets a user keeps across projects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresetsFile {
    pub format_version: u32,
    pub presets: Vec<EffectPreset>,
}

impl PresetsFile {
    pub fn new(presets: Vec<EffectPreset>) -> Self {
        Self {
            format_version: PRESETS_VERSION,
            presets,
        }
    }

    pub fn to_text(&self) -> Result<String, ProjectError> {
        Ok(json::to_value(self)?.to_text())
    }

    pub fn from_text(text: &str) -> Result<Self, ProjectError> {
        let document = Value::parse(text)?;
        format_version(&document, PRESETS_VERSION)?;
        let mut file: Self = json::from_value(document)?;
        file.format_version = PRESETS_VERSION;
        Ok(file)
    }
}
