use serde::{Deserialize, Serialize};

use crate::{Param, Rgb};

/// Paints the path the keyframed `brush_position`, in percent of the picture, has
/// taken over the last `stroke_length` seconds, or since the clip began at 0.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WriteOn {
    pub brush_position: [Param; 2],
    pub color: Rgb,
    /// Pixels across.
    pub brush_size: Param,
    /// Percent of the brush that is solid before its edge fades.
    pub brush_hardness: Param,
    /// Percent.
    pub brush_opacity: Param,
    /// Seconds.
    pub stroke_length: Param,
    /// Seconds between the points the stroke is drawn through.
    pub brush_spacing: Param,
    pub paint_style: WriteOnStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum WriteOnStyle {
    #[default]
    OnOriginalImage,
    OnTransparent,
    /// The stroke uncovers the picture, which is otherwise hidden.
    RevealOriginalImage,
}

impl WriteOnStyle {
    pub const ALL: [Self; 3] = [
        Self::OnOriginalImage,
        Self::OnTransparent,
        Self::RevealOriginalImage,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::OnOriginalImage => "On Original Image",
            Self::OnTransparent => "On Transparent",
            Self::RevealOriginalImage => "Reveal Original Image",
        }
    }
}

impl Default for WriteOn {
    fn default() -> Self {
        Self {
            brush_position: [Param::from_value(50.0), Param::from_value(50.0)],
            color: Rgb::WHITE,
            brush_size: Param::from_value(5.0),
            brush_hardness: Param::from_value(75.0),
            brush_opacity: Param::from_value(100.0),
            stroke_length: Param::default(),
            brush_spacing: Param::from_value(0.01),
            paint_style: WriteOnStyle::default(),
        }
    }
}
