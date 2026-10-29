//! Lighting Effects: up to five lights over the picture as a surface, with ambient
//! light, gloss and an optional bump from the picture's own brightness.

use serde::{Deserialize, Serialize};

use super::Rgb;
use crate::Param;

pub const MAX_LIGHTS: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LightingEffects {
    /// Always `MAX_LIGHTS` of them; held apart so the effect stays small in the enum.
    pub lights: Vec<Light>,
    pub ambience_color: Rgb,
    /// −100 to 100, as are the surface settings and the exposure.
    pub ambience_intensity: Param,
    pub surface_gloss: Param,
    /// From plastic, highlights in the light's color, to metal, in the picture's.
    pub surface_material: Param,
    pub exposure: Param,
    /// 0 for a flat surface; above it, how much the picture's brightness raises it.
    pub bump_height: Param,
    /// Bright parts of the picture stand out of the surface rather than sink into it.
    pub white_is_high: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Light {
    pub kind: LightKind,
    pub color: Rgb,
    /// Percent of the picture's width and height.
    pub center: [Param; 2],
    /// Percent of the picture's width.
    pub major_radius: Param,
    pub minor_radius: Param,
    /// Degrees.
    pub angle: Param,
    /// −100 to 100, as is the focus.
    pub intensity: Param,
    /// How sharp the spotlight's edge is.
    pub focus: Param,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LightKind {
    #[default]
    None,
    /// From one direction, evenly across the picture.
    Directional,
    /// From a point just over the picture, in every direction.
    Omni,
    Spotlight,
}

impl LightKind {
    pub const ALL: [Self; 4] = [Self::None, Self::Directional, Self::Omni, Self::Spotlight];

    pub fn name(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Directional => "Directional",
            Self::Omni => "Omni",
            Self::Spotlight => "Spotlight",
        }
    }
}

impl Default for Light {
    fn default() -> Self {
        Self {
            kind: LightKind::None,
            color: Rgb::WHITE,
            center: [Param::from_value(50.0), Param::from_value(50.0)],
            major_radius: Param::from_value(30.0),
            minor_radius: Param::from_value(20.0),
            angle: Param::from_value(30.0),
            intensity: Param::from_value(20.0),
            focus: Param::from_value(20.0),
        }
    }
}

impl Default for LightingEffects {
    fn default() -> Self {
        let mut lights = vec![Light::default(); MAX_LIGHTS];
        lights[0].kind = LightKind::Spotlight;
        Self {
            lights,
            ambience_color: Rgb::WHITE,
            ambience_intensity: Param::from_value(20.0),
            surface_gloss: Param::default(),
            surface_material: Param::default(),
            exposure: Param::default(),
            bump_height: Param::default(),
            white_is_high: true,
        }
    }
}
