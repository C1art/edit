//! Noise HLS Auto and Noise Alpha.

use serde::{Deserialize, Serialize};

use crate::Param;

/// Noise HLS whose pattern changes by itself, `animation_speed` new patterns a second.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NoiseHlsAuto {
    pub hue: Param,
    pub lightness: Param,
    pub saturation: Param,
    pub grain_size: Param,
    pub animation_speed: Param,
}

/// Random opacity per pixel, mixed into the picture's own.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NoiseAlpha {
    pub noise: AlphaNoise,
    /// Percent.
    pub amount: Param,
    pub original_alpha: OriginalAlpha,
    pub overflow: AlphaOverflow,
    pub random_seed: u32,
    /// Degrees; each turn moves an animated noise on to a new pattern.
    pub noise_phase: Param,
    /// Repeats the animated noise after `cycle` turns of the phase.
    pub cycle_noise: bool,
    pub cycle: Param,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum AlphaNoise {
    #[default]
    UniformRandom,
    /// Squared, so most pixels get little noise and a few get much.
    SquaredRandom,
    UniformAnimation,
    SquaredAnimation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum OriginalAlpha {
    /// Noise everywhere, on top of the picture's opacity.
    #[default]
    Add,
    /// Noise only where the picture is opaque.
    Clamp,
    /// More noise where the picture is more opaque.
    Scale,
    /// Noise only where the picture is transparent.
    UseNoiseAsMask,
}

/// What happens to opacity pushed past fully opaque or transparent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum AlphaOverflow {
    #[default]
    Clip,
    /// Folds back from the limit.
    WrapBack,
    /// Starts over from the other end.
    WrapAround,
}

impl AlphaNoise {
    pub const ALL: [Self; 4] = [
        Self::UniformRandom,
        Self::SquaredRandom,
        Self::UniformAnimation,
        Self::SquaredAnimation,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::UniformRandom => "Uniform Random",
            Self::SquaredRandom => "Squared Random",
            Self::UniformAnimation => "Uniform Animation",
            Self::SquaredAnimation => "Squared Animation",
        }
    }
}

impl OriginalAlpha {
    pub const ALL: [Self; 4] = [Self::Add, Self::Clamp, Self::Scale, Self::UseNoiseAsMask];

    pub fn name(self) -> &'static str {
        match self {
            Self::Add => "Add",
            Self::Clamp => "Clamp",
            Self::Scale => "Scale",
            Self::UseNoiseAsMask => "Use Noise as Mask",
        }
    }
}

impl AlphaOverflow {
    pub const ALL: [Self; 3] = [Self::Clip, Self::WrapBack, Self::WrapAround];

    pub fn name(self) -> &'static str {
        match self {
            Self::Clip => "Clip",
            Self::WrapBack => "Wrap Back",
            Self::WrapAround => "Wrap Around",
        }
    }
}

impl Default for NoiseHlsAuto {
    fn default() -> Self {
        Self {
            hue: Param::default(),
            lightness: Param::from_value(10.0),
            saturation: Param::default(),
            grain_size: Param::from_value(1.0),
            animation_speed: Param::from_value(10.0),
        }
    }
}

impl Default for NoiseAlpha {
    fn default() -> Self {
        Self {
            noise: AlphaNoise::default(),
            amount: Param::default(),
            original_alpha: OriginalAlpha::default(),
            overflow: AlphaOverflow::default(),
            random_seed: 0,
            noise_phase: Param::default(),
            cycle_noise: false,
            cycle: Param::from_value(1.0),
        }
    }
}
