use serde::{Deserialize, Serialize};
use timeline::TitleTemplate;

use crate::json::{self, Value};
use crate::{format_version, ProjectError};

/// Bump on every breaking change to the templates layout, migrating older files.
const TEMPLATES_VERSION: u32 = 1;

/// The title templates a user keeps across projects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplatesFile {
    pub format_version: u32,
    pub templates: Vec<TitleTemplate>,
}

impl TemplatesFile {
    pub fn new(templates: Vec<TitleTemplate>) -> Self {
        Self {
            format_version: TEMPLATES_VERSION,
            templates,
        }
    }

    pub fn to_text(&self) -> Result<String, ProjectError> {
        Ok(json::to_value(self)?.to_text())
    }

    pub fn from_text(text: &str) -> Result<Self, ProjectError> {
        let document = Value::parse(text)?;
        format_version(&document, TEMPLATES_VERSION)?;
        let mut file: Self = json::from_value(document)?;
        file.format_version = TEMPLATES_VERSION;
        Ok(file)
    }
}
