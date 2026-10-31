//! Effects on the picture's edges. Sizes are in the clip's pixels, angles in degrees
//! clockwise from up and amounts in percent unless said otherwise.

use serde::{Deserialize, Serialize};

use super::Rgb;
use crate::Param;

/// Wears the picture's edges, and those of its alpha, into a fractal outline.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RoughenEdges {
    pub edge_type: RoughenEdgeType,
    /// Shown along the worn edge by Roughen Color.
    pub edge_color: Rgb,
    pub border: Param,
    /// 0 to 10.
    pub edge_sharpness: Param,
    /// 0 to 1: how much the fractal shapes the edge rather than an even inset.
    pub fractal_influence: Param,
    /// Percent; 100 makes the fractal's largest features 32 pixels across.
    pub scale: Param,
    pub offset: [Param; 2],
    /// Layers of finer fractal, 1 to 10.
    pub complexity: Param,
    pub evolution: Param,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum RoughenEdgeType {
    #[default]
    Roughen,
    RoughenColor,
}

impl RoughenEdgeType {
    pub const ALL: [Self; 2] = [Self::Roughen, Self::RoughenColor];

    pub fn name(self) -> &'static str {
        match self {
            Self::Roughen => "Roughen",
            Self::RoughenColor => "Roughen Color",
        }
    }
}

/// Lights the picture's border, or the edges of its alpha, as if raised. Bevel Edges'
/// thickness is in percent of the picture's shorter side, Bevel Alpha's in pixels.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Bevel {
    pub edge_thickness: Param,
    pub light_angle: Param,
    pub light_color: Rgb,
    pub light_intensity: Param,
}

/// Color spreading out from the edges of the picture's alpha.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AlphaGlow {
    pub glow: Param,
    /// 0 to 255.
    pub brightness: Param,
    pub start_color: Rgb,
    /// The glow turns toward it at its outer edge, when set.
    pub end_color: Option<Rgb>,
    pub fade_out: bool,
}

impl Default for RoughenEdges {
    fn default() -> Self {
        Self {
            edge_type: RoughenEdgeType::default(),
            edge_color: Rgb::WHITE,
            border: Param::from_value(8.0),
            edge_sharpness: Param::from_value(1.0),
            fractal_influence: Param::from_value(1.0),
            scale: Param::from_value(100.0),
            offset: [Param::default(), Param::default()],
            complexity: Param::from_value(2.0),
            evolution: Param::default(),
        }
    }
}

impl Bevel {
    pub fn edges() -> Self {
        Self {
            edge_thickness: Param::from_value(10.0),
            light_angle: Param::from_value(-60.0),
            light_color: Rgb::WHITE,
            light_intensity: Param::from_value(25.0),
        }
    }

    pub fn alpha() -> Self {
        Self {
            edge_thickness: Param::from_value(2.0),
            light_intensity: Param::from_value(40.0),
            ..Self::edges()
        }
    }
}

impl Default for AlphaGlow {
    fn default() -> Self {
        Self {
            glow: Param::from_value(10.0),
            brightness: Param::from_value(255.0),
            start_color: Rgb::WHITE,
            end_color: None,
            fade_out: true,
        }
    }
}

/// Emboss's relief, lighting the picture's colors rather than grey.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ColorEmboss {
    pub direction: Param,
    pub relief: Param,
    pub contrast: Param,
    pub blend: Param,
}

impl Default for ColorEmboss {
    fn default() -> Self {
        Self {
            direction: Param::from_value(45.0),
            relief: Param::from_value(1.0),
            contrast: Param::from_value(100.0),
            blend: Param::default(),
        }
    }
}
