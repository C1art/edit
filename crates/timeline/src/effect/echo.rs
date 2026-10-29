//! Echo: the picture layered with copies of itself from other moments of the clip.

use serde::{Deserialize, Serialize};

use crate::Param;

/// Layers the picture with `echoes` copies of itself, each `echo_time` sequence frames
/// further along the clip; negative times look back. The picture itself shows at
/// `starting_intensity` (0 to 1), and each echo at `decay` times the one before.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Echo {
    pub echo_time: Param,
    pub echoes: Param,
    pub starting_intensity: Param,
    pub decay: Param,
    pub operator: EchoOperator,
}

/// How Echo combines the picture with its echoes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum EchoOperator {
    #[default]
    Add,
    Maximum,
    Minimum,
    Screen,
    /// Each echo drawn behind the one before it, by its alpha.
    CompositeInBack,
    /// Each echo drawn over the one before it, by its alpha.
    CompositeInFront,
    /// The average of the picture and its echoes.
    Blend,
}

impl EchoOperator {
    pub const ALL: [Self; 7] = [
        Self::Add,
        Self::Maximum,
        Self::Minimum,
        Self::Screen,
        Self::CompositeInBack,
        Self::CompositeInFront,
        Self::Blend,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Add => "Add",
            Self::Maximum => "Maximum",
            Self::Minimum => "Minimum",
            Self::Screen => "Screen",
            Self::CompositeInBack => "Composite In Back",
            Self::CompositeInFront => "Composite In Front",
            Self::Blend => "Blend",
        }
    }
}

impl Default for Echo {
    fn default() -> Self {
        Self {
            echo_time: Param::from_value(-1.0),
            echoes: Param::from_value(1.0),
            starting_intensity: Param::from_value(1.0),
            decay: Param::from_value(1.0),
            operator: EchoOperator::Add,
        }
    }
}

/// Blurs each of the picture's red, green, blue and alpha channels by its own
/// blurriness, in sequence pixels.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct ChannelBlur {
    pub red: Param,
    pub green: Param,
    pub blue: Param,
    pub alpha: Param,
}
