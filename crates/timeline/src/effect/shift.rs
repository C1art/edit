//! Change Color and Color Balance, which shift colors by where they sit: near a chosen
//! color, or in the shadows, midtones or highlights.

use serde::{Deserialize, Serialize};

use super::Rgb;
use crate::Param;

/// Shifts the hue, lightness and saturation of the colors near one color.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChangeColor {
    pub view: ChangeColorView,
    /// Degrees.
    pub hue_transform: Param,
    /// −100 to 100.
    pub lightness_transform: Param,
    pub saturation_transform: Param,
    pub color_to_change: Rgb,
    /// Percent.
    pub matching_tolerance: Param,
    pub matching_softness: Param,
    pub match_colors: MatchColors,
    pub invert_mask: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChangeColorView {
    #[default]
    CorrectedLayer,
    /// The matched colors in white, to judge the tolerance by.
    ColorCorrectionMask,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MatchColors {
    #[default]
    Rgb,
    Hue,
    /// By hue and saturation together, ignoring brightness.
    Chroma,
}

/// Pushes red, green and blue up or down in the shadows, midtones and highlights,
/// −100 to 100 each.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ColorBalance {
    pub shadows: [Param; 3],
    pub midtones: [Param; 3],
    pub highlights: [Param; 3],
    pub preserve_luminosity: bool,
}

impl ChangeColorView {
    pub const ALL: [Self; 2] = [Self::CorrectedLayer, Self::ColorCorrectionMask];

    pub fn name(self) -> &'static str {
        match self {
            Self::CorrectedLayer => "Corrected Layer",
            Self::ColorCorrectionMask => "Color Correction Mask",
        }
    }
}

impl MatchColors {
    pub const ALL: [Self; 3] = [Self::Rgb, Self::Hue, Self::Chroma];

    pub fn name(self) -> &'static str {
        match self {
            Self::Rgb => "Using RGB",
            Self::Hue => "Using Hue",
            Self::Chroma => "Using Chroma",
        }
    }
}

impl Default for ChangeColor {
    fn default() -> Self {
        Self {
            view: ChangeColorView::default(),
            hue_transform: Param::default(),
            lightness_transform: Param::default(),
            saturation_transform: Param::default(),
            color_to_change: Rgb::new(255, 0, 0),
            matching_tolerance: Param::from_value(15.0),
            matching_softness: Param::default(),
            match_colors: MatchColors::default(),
            invert_mask: false,
        }
    }
}

impl Default for ColorBalance {
    fn default() -> Self {
        let none = || std::array::from_fn(|_| Param::default());
        Self {
            shadows: none(),
            midtones: none(),
            highlights: none(),
            preserve_luminosity: false,
        }
    }
}
