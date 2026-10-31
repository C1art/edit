use serde::{Deserialize, Serialize};

use crate::{Param, Rgb};

/// Fills the area around `fill_point`, in percent of the picture, that is within
/// `tolerance` of the color there, as far as it reaches without a break.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PaintBucket {
    pub fill_point: [Param; 2],
    pub fill_selector: FillSelector,
    /// 0 to 255 on each channel.
    pub tolerance: Param,
    /// Shows the filled area white on black, to set the tolerance by.
    pub view_threshold: bool,
    /// Pixels the edge of the fill grows by, or shrinks by when negative.
    pub spread: Param,
    /// Pixels the edge of the fill softens over.
    pub feather: Param,
    pub color: Rgb,
    /// Percent.
    pub opacity: Param,
    pub blending_mode: BucketBlend,
}

/// What two pixels are compared on to tell whether the fill reaches from one to the
/// other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum FillSelector {
    #[default]
    ColorAndAlpha,
    StraightColor,
    /// Fills only the see-through area around the point.
    Transparency,
    /// Fills only the solid area around the point.
    Opacity,
}

impl FillSelector {
    pub const ALL: [Self; 4] = [
        Self::ColorAndAlpha,
        Self::StraightColor,
        Self::Transparency,
        Self::Opacity,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::ColorAndAlpha => "Color & Alpha",
            Self::StraightColor => "Straight Color",
            Self::Transparency => "Transparency",
            Self::Opacity => "Opacity",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum BucketBlend {
    #[default]
    Normal,
    /// The fill shows only where the picture is see-through.
    Behind,
    /// The fill alone, with the picture left out.
    FillOnly,
}

impl BucketBlend {
    pub const ALL: [Self; 3] = [Self::Normal, Self::Behind, Self::FillOnly];

    pub fn name(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Behind => "Behind",
            Self::FillOnly => "Fill Only",
        }
    }
}

impl Default for PaintBucket {
    fn default() -> Self {
        Self {
            fill_point: [Param::from_value(50.0), Param::from_value(50.0)],
            fill_selector: FillSelector::default(),
            tolerance: Param::from_value(10.0),
            view_threshold: false,
            spread: Param::default(),
            feather: Param::default(),
            color: Rgb::WHITE,
            opacity: Param::from_value(100.0),
            blending_mode: BucketBlend::default(),
        }
    }
}
