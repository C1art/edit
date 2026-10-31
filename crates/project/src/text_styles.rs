use serde::{Deserialize, Serialize};
use timeline::TextStyle;

use crate::json::{self, Value};
use crate::{format_version, ProjectError};

/// Bump on every breaking change to the text styles layout, migrating older files.
const TEXT_STYLES_VERSION: u32 = 1;

/// The text styles a user keeps across projects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextStylesFile {
    pub format_version: u32,
    pub styles: Vec<TextStyle>,
}

impl TextStylesFile {
    pub fn new(styles: Vec<TextStyle>) -> Self {
        Self {
            format_version: TEXT_STYLES_VERSION,
            styles,
        }
    }

    pub fn to_text(&self) -> Result<String, ProjectError> {
        Ok(json::to_value(self)?.to_text())
    }

    pub fn from_text(text: &str) -> Result<Self, ProjectError> {
        let document = Value::parse(text)?;
        format_version(&document, TEXT_STYLES_VERSION)?;
        let mut file: Self = json::from_value(document)?;
        file.format_version = TEXT_STYLES_VERSION;
        Ok(file)
    }
}
