//! Brush Strokes, Eyedropper Fill and SDR Conform.

use serde::{Deserialize, Serialize};

use crate::Param;

/// Repaints the picture in short strokes of its own colors, `brush_size` and
/// `stroke_length` pixels, running at `stroke_angle` degrees and each turned up to
/// `stroke_randomness` percent of a half turn away from it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BrushStrokes {
    pub stroke_angle: Param,
    pub brush_size: Param,
    pub stroke_length: Param,
    pub stroke_randomness: Param,
    pub paint_surface: PaintSurface,
    /// Percent.
    pub blend_with_original: Param,
}

/// What the strokes are painted onto: the picture itself, or nothing, white or black
/// showing between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum PaintSurface {
    #[default]
    OnOriginalImage,
    OnTransparent,
    OnWhite,
    OnBlack,
}

impl PaintSurface {
    pub const ALL: [Self; 4] = [
        Self::OnOriginalImage,
        Self::OnTransparent,
        Self::OnWhite,
        Self::OnBlack,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::OnOriginalImage => "Paint On Original Image",
            Self::OnTransparent => "Paint On Transparent",
            Self::OnWhite => "Paint On White",
            Self::OnBlack => "Paint On Black",
        }
    }
}

/// Fills the picture with the color it has around `sample_point`, in percent of the
/// picture, averaged over `sample_radius` pixels.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EyedropperFill {
    pub sample_point: [Param; 2],
    pub sample_radius: Param,
    /// Keeps the picture's own transparency instead of filling it solid.
    pub maintain_original_alpha: bool,
    /// Percent.
    pub blend_with_original: Param,
}

/// Brings high dynamic range pictures into standard range: `brightness` and `contrast`
/// from −100 to 100, and `soft_knee` percent of the range easing highlights into white
/// instead of clipping them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SdrConform {
    pub brightness: Param,
    pub contrast: Param,
    pub soft_knee: Param,
}

impl Default for BrushStrokes {
    fn default() -> Self {
        Self {
            stroke_angle: Param::from_value(135.0),
            brush_size: Param::from_value(2.0),
            stroke_length: Param::from_value(15.0),
            stroke_randomness: Param::from_value(10.0),
            paint_surface: PaintSurface::default(),
            blend_with_original: Param::default(),
        }
    }
}

impl Default for EyedropperFill {
    fn default() -> Self {
        Self {
            sample_point: [Param::from_value(50.0), Param::from_value(50.0)],
            sample_radius: Param::default(),
            maintain_original_alpha: true,
            blend_with_original: Param::default(),
        }
    }
}

impl Default for SdrConform {
    fn default() -> Self {
        Self {
            brightness: Param::default(),
            contrast: Param::default(),
            soft_knee: Param::from_value(20.0),
        }
    }
}
