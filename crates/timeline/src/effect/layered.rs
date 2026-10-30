//! Difference Matte, Compound Blur and Texturize, which each read the picture of another
//! video track, as Track Matte Key does, by its index among the video tracks the clip
//! sits with, bottom first.

use serde::{Deserialize, Serialize};

use crate::Param;

/// Keys out where the clip matches the difference track, such as a clean plate of the
/// same shot, in both position and color.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DifferenceMatte {
    pub view: DifferenceView,
    pub difference_layer: Option<usize>,
    /// Percent.
    pub matching_tolerance: Param,
    /// Percent.
    pub matching_softness: Param,
    /// Pixels; evens out noise and grain before the two are compared.
    pub blur_before_difference: Param,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum DifferenceView {
    #[default]
    FinalOutput,
    SourceOnly,
    MatteOnly,
}

impl DifferenceView {
    pub const ALL: [Self; 3] = [Self::FinalOutput, Self::SourceOnly, Self::MatteOnly];

    pub fn name(self) -> &'static str {
        match self {
            Self::FinalOutput => "Final Output",
            Self::SourceOnly => "Source Only",
            Self::MatteOnly => "Matte Only",
        }
    }
}

/// Blurs the clip more where the blur track is brighter, up to `maximum_blur` pixels.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CompoundBlur {
    pub blur_layer: Option<usize>,
    pub maximum_blur: Param,
    /// Blurs where the blur track is darker instead.
    pub invert_blur: bool,
}

/// Lights the clip as though it were embossed with the brightness of the texture track.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Texturize {
    pub texture_layer: Option<usize>,
    /// Degrees clockwise from up.
    pub light_direction: Param,
    /// 0 to 2.
    pub texture_contrast: Param,
}

impl Default for DifferenceMatte {
    fn default() -> Self {
        Self {
            view: DifferenceView::default(),
            difference_layer: None,
            matching_tolerance: Param::from_value(15.0),
            matching_softness: Param::default(),
            blur_before_difference: Param::default(),
        }
    }
}

impl Default for CompoundBlur {
    fn default() -> Self {
        Self {
            blur_layer: None,
            maximum_blur: Param::from_value(20.0),
            invert_blur: false,
        }
    }
}

impl Default for Texturize {
    fn default() -> Self {
        Self {
            texture_layer: None,
            light_direction: Param::from_value(135.0),
            texture_contrast: Param::from_value(1.0),
        }
    }
}

impl crate::VideoEffect {
    pub(crate) fn other_track(&self) -> Option<usize> {
        match self {
            Self::TrackMatteKey { matte, .. } | Self::SetMatte { matte, .. } => *matte,
            Self::DifferenceMatte(effect) => effect.difference_layer,
            Self::CompoundBlur(effect) => effect.blur_layer,
            Self::Texturize(effect) => effect.texture_layer,
            _ => None,
        }
    }

    /// The video track this effect reads besides the clip's own, for keeping it pointed
    /// at the same track as tracks below it come and go.
    pub(crate) fn other_track_mut(&mut self) -> Option<&mut Option<usize>> {
        match self {
            Self::TrackMatteKey { matte, .. } | Self::SetMatte { matte, .. } => Some(matte),
            Self::DifferenceMatte(effect) => Some(&mut effect.difference_layer),
            Self::CompoundBlur(effect) => Some(&mut effect.blur_layer),
            Self::Texturize(effect) => Some(&mut effect.texture_layer),
            _ => None,
        }
    }
}
