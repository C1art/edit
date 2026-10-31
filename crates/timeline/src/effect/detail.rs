//! Effects that work on the picture's fine detail, their sizes in the picture's pixels.

use serde::{Deserialize, Serialize};

use crate::Param;

/// Sharpens by adding back the difference from a blur `radius` pixels wide, where that
/// difference passes `threshold` levels (0 to 255).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UnsharpMask {
    /// Percent.
    pub amount: Param,
    pub radius: Param,
    pub threshold: Param,
}

/// Replaces each pixel with the median of its neighbors `radius` pixels away. With a
/// threshold, as Dust & Scratches, only pixels that far (0 to 255 levels) from the
/// median are replaced.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Median {
    pub radius: Param,
    pub threshold: Param,
}

/// Fades the picture out toward its edges over `amount` pixels.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EdgeFeather {
    pub amount: Param,
}

/// Random shifts of each pixel's hue, lightness and saturation, in percent, over grains
/// `grain_size` pixels across; the pattern changes as `phase` (degrees) turns.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NoiseHls {
    pub hue: Param,
    pub lightness: Param,
    pub saturation: Param,
    pub grain_size: Param,
    pub phase: Param,
}

impl Default for UnsharpMask {
    fn default() -> Self {
        Self {
            amount: Param::from_value(50.0),
            radius: Param::from_value(1.0),
            threshold: Param::default(),
        }
    }
}

impl Median {
    pub fn new(radius: f32) -> Self {
        Self {
            radius: Param::from_value(radius),
            threshold: Param::default(),
        }
    }
}

impl Default for NoiseHls {
    fn default() -> Self {
        Self {
            hue: Param::default(),
            lightness: Param::from_value(10.0),
            saturation: Param::default(),
            grain_size: Param::from_value(1.0),
            phase: Param::default(),
        }
    }
}
