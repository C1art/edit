use serde::{Deserialize, Serialize};

use crate::Rgb;

/// A drop shadow's look, as Essential Graphics sets it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ShadowStyle {
    pub color: Rgb,
    /// Percent.
    pub opacity: u8,
    /// Degrees clockwise from straight up toward which the shadow falls.
    pub angle: i16,
    /// In sequence pixels.
    pub distance: u32,
    /// How far the shadow spreads past the silhouette, in sequence pixels.
    pub size: u32,
    pub blur: u32,
}

impl ShadowStyle {
    /// A black shadow falling down and to the right, Premiere's 135 degrees.
    pub const fn falling(distance: u32, blur: u32) -> Self {
        Self {
            color: Rgb::BLACK,
            opacity: 70,
            angle: 135,
            distance,
            size: 0,
            blur,
        }
    }

    /// How far the shadow sits from what casts it, in pixels right and down.
    pub fn offset(self) -> (i64, i64) {
        let (sin, cos) = f64::from(self.angle).to_radians().sin_cos();
        let distance = f64::from(self.distance);
        (
            (sin * distance).round() as i64,
            (-cos * distance).round() as i64,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_angle_turns_clockwise_from_up() {
        let at = |angle| ShadowStyle {
            angle,
            ..ShadowStyle::falling(10, 0)
        };
        assert_eq!(at(135).offset(), (7, 7));
        assert_eq!(at(0).offset(), (0, -10));
        assert_eq!(at(270).offset(), (-10, 0));
    }
}
