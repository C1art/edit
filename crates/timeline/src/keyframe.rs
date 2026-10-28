use serde::{Deserialize, Serialize};

use crate::Ticks;

/// A value a parameter passes through at one source time of its clip, so keyframes
/// stay on the same picture when the clip is trimmed, split or moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Keyframe {
    /// In the clip's source time, in the sequence timebase.
    pub time: Ticks,
    pub hundredths: i32,
    #[serde(default, skip_serializing_if = "Interpolation::is_linear")]
    pub interpolation: Interpolation,
    /// The slopes a Bezier keyframe's curve arrives and leaves with, once set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handles: Option<Handles>,
}

impl Keyframe {
    pub const fn new(time: Ticks, hundredths: i32) -> Self {
        Self {
            time,
            hundredths,
            interpolation: Interpolation::Linear,
            handles: None,
        }
    }

    pub fn value(&self) -> f32 {
        self.hundredths as f32 / 100.0
    }
}

/// How the value moves through a keyframe. Easing slows the change arriving at the
/// keyframe (in), leaving it (out) or both; a hold keeps the value until the next one;
/// auto Bezier curves smoothly through it, toward its neighbours; Bezier follows handles
/// set apart on each side, continuous Bezier handles that stay in one line.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Interpolation {
    #[default]
    Linear,
    Hold,
    EaseIn,
    EaseOut,
    Ease,
    AutoBezier,
    Bezier,
    ContinuousBezier,
}

impl Interpolation {
    pub const ALL: [Self; 8] = [
        Self::Linear,
        Self::Bezier,
        Self::AutoBezier,
        Self::ContinuousBezier,
        Self::Hold,
        Self::EaseIn,
        Self::EaseOut,
        Self::Ease,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Linear => "Linear",
            Self::Hold => "Hold",
            Self::EaseIn => "Ease In",
            Self::EaseOut => "Ease Out",
            Self::Ease => "Ease In and Out",
            Self::AutoBezier => "Auto Bezier",
            Self::Bezier => "Bezier",
            Self::ContinuousBezier => "Continuous Bezier",
        }
    }

    pub fn has_handles(self) -> bool {
        matches!(self, Self::Bezier | Self::ContinuousBezier)
    }

    fn is_linear(&self) -> bool {
        *self == Self::Linear
    }

    pub(crate) fn eases_in(self) -> bool {
        matches!(self, Self::EaseIn | Self::Ease)
    }

    pub(crate) fn eases_out(self) -> bool {
        matches!(self, Self::EaseOut | Self::Ease)
    }
}

/// Billionths of a hundredth per tick, small enough for a slow change at a fine timebase.
const SLOPE_SCALE: f64 = 1e9;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HandleSide {
    Incoming,
    Outgoing,
}

/// A Bezier keyframe's slopes in hundredths per tick, held as integers so keyframes stay
/// `Eq`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Handles {
    incoming: i64,
    outgoing: i64,
}

impl Handles {
    pub fn level(slope: f64) -> Self {
        let slope = (slope * SLOPE_SCALE).round() as i64;
        Self {
            incoming: slope,
            outgoing: slope,
        }
    }

    pub fn slope(self, side: HandleSide) -> f64 {
        let slope = match side {
            HandleSide::Incoming => self.incoming,
            HandleSide::Outgoing => self.outgoing,
        };
        slope as f64 / SLOPE_SCALE
    }

    pub fn set(&mut self, side: HandleSide, slope: f64) {
        let slope = (slope * SLOPE_SCALE).round() as i64;
        match side {
            HandleSide::Incoming => self.incoming = slope,
            HandleSide::Outgoing => self.outgoing = slope,
        }
    }
}
