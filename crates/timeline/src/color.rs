use serde::{Deserialize, Serialize};

use crate::Param;

/// Basic color correction, applied in linear light before the clip is placed.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorCorrection {
    /// Temperature, -100 (cool) to 100 (warm).
    pub temperature: Param,
    /// -100 (green) to 100 (magenta).
    pub tint: Param,
    /// Stops.
    pub exposure: Param,
    /// -100 to 100.
    pub contrast: Param,
    /// -100 to 100.
    pub highlights: Param,
    /// -100 to 100.
    pub shadows: Param,
    /// Percent; 0 is monochrome.
    pub saturation: Param,
}

impl Default for ColorCorrection {
    fn default() -> Self {
        Self {
            temperature: Param::default(),
            tint: Param::default(),
            exposure: Param::default(),
            contrast: Param::default(),
            highlights: Param::default(),
            shadows: Param::default(),
            saturation: Param::new(10_000),
        }
    }
}

impl ColorCorrection {
    pub fn params(&self) -> [&Param; 7] {
        [
            &self.temperature,
            &self.tint,
            &self.exposure,
            &self.contrast,
            &self.highlights,
            &self.shadows,
            &self.saturation,
        ]
    }
}
