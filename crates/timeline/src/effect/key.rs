use serde::{Deserialize, Serialize};

use crate::Param;

/// Pixels of the source that Ultra Key's choke and soften reach at 100%.
pub const ULTRA_KEY_REACH: f32 = 10.0;

/// What Ultra Key shows: the keyed picture, or one of the views used to judge it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyOutput {
    #[default]
    Composite,
    /// The matte in black and white, opaque.
    AlphaChannel,
    /// The spill-suppressed color, unkeyed.
    ColorChannel,
}

impl KeyOutput {
    pub const ALL: [Self; 3] = [Self::Composite, Self::AlphaChannel, Self::ColorChannel];

    pub fn name(self) -> &'static str {
        match self {
            Self::Composite => "Composite",
            Self::AlphaChannel => "Alpha Channel",
            Self::ColorChannel => "Color Channel",
        }
    }
}

// Projects from before spill suppression had the edge desaturation it now scales.
pub(super) fn default_spill() -> Param {
    Param::from_value(50.0)
}

/// What of the matte's picture decides how much of the clip shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MatteChannel {
    #[default]
    Alpha,
    /// The brightness of the matte over black.
    Luma,
}

impl MatteChannel {
    pub const ALL: [Self; 2] = [Self::Alpha, Self::Luma];

    pub fn name(self) -> &'static str {
        match self {
            Self::Alpha => "Matte Alpha",
            Self::Luma => "Matte Luma",
        }
    }
}
