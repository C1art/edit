mod error;

use serde::{Deserialize, Serialize};
use timeline::Sequence;

pub use error::ProjectError;

/// Bump on every breaking change to the file layout and add a migration from the previous version.
pub const FORMAT_VERSION: u32 = 1;

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
}
