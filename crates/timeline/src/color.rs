use serde::{Deserialize, Serialize};

use crate::Param;

/// Color correction in the manner of a grading panel, applied in linear light before
/// the clip is placed: white balance, tone, wheels for the shadows, midtones and
/// highlights, then saturation.
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
    /// -100 to 100: the brightest tones.
    pub whites: Param,
    /// -100 to 100: the black point.
    pub blacks: Param,
    pub shadow_wheel: ColorWheel,
    pub midtone_wheel: ColorWheel,
    pub highlight_wheel: ColorWheel,
    /// Percent; 0 is monochrome.
    pub saturation: Param,
    /// -100 to 100: saturates muted colors more than vivid ones.
    pub vibrance: Param,
}

/// A shift of one tonal range toward a hue, with a brightness offset.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorWheel {
    /// -100 to 100 toward blue (positive) or yellow, as Cb.
    pub blue: Param,
    /// -100 to 100 toward red (positive) or cyan, as Cr.
    pub red: Param,
    /// -100 to 100.
    pub level: Param,
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
            whites: Param::default(),
            blacks: Param::default(),
            shadow_wheel: ColorWheel::default(),
            midtone_wheel: ColorWheel::default(),
            highlight_wheel: ColorWheel::default(),
            saturation: Param::new(10_000),
            vibrance: Param::default(),
        }
    }
}

impl ColorCorrection {
    pub fn params(&self) -> impl Iterator<Item = &Param> {
        let wheels = [
            &self.shadow_wheel,
            &self.midtone_wheel,
            &self.highlight_wheel,
        ];
        [
            &self.temperature,
            &self.tint,
            &self.exposure,
            &self.contrast,
            &self.highlights,
            &self.shadows,
            &self.whites,
            &self.blacks,
            &self.saturation,
            &self.vibrance,
        ]
        .into_iter()
        .chain(wheels.into_iter().flat_map(ColorWheel::params))
    }
}

impl ColorWheel {
    pub fn params(&self) -> [&Param; 3] {
        [&self.blue, &self.red, &self.level]
    }
}
