use serde::{Deserialize, Serialize};

use super::Rgb;
use crate::Param;

/// A shadow cast from a point light, so it grows away from the light rather than
/// shifting by a fixed distance like Drop Shadow's.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RadialShadow {
    pub shadow_color: Rgb,
    pub opacity: Param,
    /// Percent of the picture's width and height.
    pub light_source: [Param; 2],
    /// How far behind the picture the shadow falls, in percent of its size.
    pub projection_distance: Param,
    /// Pixels.
    pub softness: Param,
    pub render: ShadowRender,
    /// Percent of the picture's color a glass edge shadow takes on.
    pub color_influence: Param,
    pub shadow_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum ShadowRender {
    #[default]
    Regular,
    /// Tinted by the picture, as light through colored glass.
    GlassEdge,
}

impl ShadowRender {
    pub const ALL: [Self; 2] = [Self::Regular, Self::GlassEdge];

    pub fn name(self) -> &'static str {
        match self {
            Self::Regular => "Regular",
            Self::GlassEdge => "Glass Edge",
        }
    }
}

impl Default for RadialShadow {
    fn default() -> Self {
        Self {
            shadow_color: Rgb::BLACK,
            opacity: Param::from_value(50.0),
            light_source: [Param::from_value(25.0), Param::from_value(25.0)],
            projection_distance: Param::from_value(10.0),
            softness: Param::default(),
            render: ShadowRender::default(),
            color_influence: Param::from_value(100.0),
            shadow_only: false,
        }
    }
}
