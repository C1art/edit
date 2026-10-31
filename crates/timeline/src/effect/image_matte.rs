use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Keys the clip through a still image file, stretched over the picture.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct ImageMatteKey {
    pub matte: Option<PathBuf>,
    pub composite_using: ImageMatteSource,
    /// Shows the clip where the matte would hide it, and the other way round.
    pub reverse: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum ImageMatteSource {
    #[default]
    MatteAlpha,
    MatteLuma,
}

impl ImageMatteSource {
    pub const ALL: [Self; 2] = [Self::MatteAlpha, Self::MatteLuma];

    pub fn name(self) -> &'static str {
        match self {
            Self::MatteAlpha => "Matte Alpha",
            Self::MatteLuma => "Matte Luma",
        }
    }
}
