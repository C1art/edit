//! Effects that remap the picture's tones and colors pixel by pixel or from a small
//! neighborhood. Levels run 0 to 255 and amounts are in percent unless said otherwise.

use serde::{Deserialize, Serialize};

use super::Rgb;
use crate::Param;

/// Each pixel replaced by the weighted sum of its 3×3 neighbors, divided by the scale
/// and raised by the offset.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConvolutionKernel {
    /// Row by row from the top left.
    pub kernel: [Param; 9],
    pub offset: Param,
    pub scale: Param,
}

/// White where the picture's luma falls between the levels and black elsewhere.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Extract {
    pub black_input_level: Param,
    pub white_input_level: Param,
    pub softness: Param,
    pub invert: bool,
}

/// Lifts the shadows and pulls down the highlights by how dark or bright the
/// surroundings are, rather than the pixel alone.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ShadowHighlight {
    pub shadow_amount: Param,
    pub highlight_amount: Param,
    /// How far into the midtones each reaches.
    pub shadow_tonal_width: Param,
    pub highlight_tonal_width: Param,
    /// In pixels: how wide the surroundings are.
    pub radius: Param,
    /// −100 to 100.
    pub midtone_contrast: Param,
    pub blend_with_original: Param,
}

/// Keeps the signal under a broadcast-safe level.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VideoLimiter {
    /// Percent of full scale, 90 to 110.
    pub clip_level: Param,
    pub method: LimiterMethod,
    /// How far below the clip level, in percent of it, the signal starts easing in.
    pub compression_before_clipping: Param,
    /// Shows what the limiter changes in this color instead of correcting it.
    pub gamut_warning: Option<Rgb>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum LimiterMethod {
    ReduceLuma,
    ReduceChroma,
    ReduceChromaAndLuma,
    #[default]
    SmartLimit,
}

impl LimiterMethod {
    pub const ALL: [Self; 4] = [
        Self::ReduceLuma,
        Self::ReduceChroma,
        Self::ReduceChromaAndLuma,
        Self::SmartLimit,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::ReduceLuma => "Reduce Luma",
            Self::ReduceChroma => "Reduce Chroma",
            Self::ReduceChromaAndLuma => "Reduce Chroma and Luma",
            Self::SmartLimit => "Smart Limit",
        }
    }
}

/// The ASC Color Decision List's slope, offset and power per channel, then saturation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AscCdl {
    pub slope: [Param; 3],
    pub offset: [Param; 3],
    pub power: [Param; 3],
    pub saturation: Param,
}

/// Colors near the target turned into the replacement, keeping their shading unless
/// solid.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ColorReplace {
    pub target_color: Rgb,
    pub replace_color: Rgb,
    pub similarity: Param,
    pub solid_colors: bool,
}

impl Default for ConvolutionKernel {
    fn default() -> Self {
        let mut kernel = std::array::from_fn(|_| Param::default());
        kernel[4] = Param::from_value(1.0);
        Self {
            kernel,
            offset: Param::default(),
            scale: Param::from_value(1.0),
        }
    }
}

impl Default for Extract {
    fn default() -> Self {
        Self {
            black_input_level: Param::from_value(64.0),
            white_input_level: Param::from_value(255.0),
            softness: Param::default(),
            invert: false,
        }
    }
}

impl Default for ShadowHighlight {
    fn default() -> Self {
        Self {
            shadow_amount: Param::from_value(50.0),
            highlight_amount: Param::default(),
            shadow_tonal_width: Param::from_value(50.0),
            highlight_tonal_width: Param::from_value(50.0),
            radius: Param::from_value(30.0),
            midtone_contrast: Param::default(),
            blend_with_original: Param::default(),
        }
    }
}

impl Default for VideoLimiter {
    fn default() -> Self {
        Self {
            clip_level: Param::from_value(100.0),
            method: LimiterMethod::default(),
            compression_before_clipping: Param::from_value(3.0),
            gamut_warning: None,
        }
    }
}

impl Default for AscCdl {
    fn default() -> Self {
        let ones = || std::array::from_fn(|_| Param::from_value(1.0));
        Self {
            slope: ones(),
            offset: std::array::from_fn(|_| Param::default()),
            power: ones(),
            saturation: Param::from_value(1.0),
        }
    }
}

impl Default for ColorReplace {
    fn default() -> Self {
        Self {
            target_color: Rgb::WHITE,
            replace_color: Rgb::BLACK,
            similarity: Param::from_value(10.0),
            solid_colors: false,
        }
    }
}
