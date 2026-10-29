//! Color Pass, Reduce Interlace Flicker and Cineon Converter.

use serde::{Deserialize, Serialize};

use super::Rgb;
use crate::Param;

/// Turns everything but `color` grey, or with `reverse` only `color`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ColorPass {
    pub color: Rgb,
    /// Percent.
    pub similarity: Param,
    pub reverse: bool,
}

/// Softens fine horizontal detail that twitters on interlaced displays.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReduceInterlaceFlicker {
    /// Lines.
    pub softness: Param,
}

/// Converts between Kodak Cineon logarithmic film scans and linear video, with the
/// points in 10-bit code values and the internal ones in percent.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CineonConverter {
    pub conversion: CineonConversion,
    pub black_point_10_bit: Param,
    pub internal_black_point: Param,
    pub white_point_10_bit: Param,
    pub internal_white_point: Param,
    pub gamma: Param,
    /// Code values below the white point where highlights start easing into it.
    pub highlight_rolloff: Param,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum CineonConversion {
    #[default]
    LogToLinear,
    LinearToLog,
    LogToLog,
}

impl CineonConversion {
    pub const ALL: [Self; 3] = [Self::LogToLinear, Self::LinearToLog, Self::LogToLog];

    pub fn name(self) -> &'static str {
        match self {
            Self::LogToLinear => "Log to Linear",
            Self::LinearToLog => "Linear to Log",
            Self::LogToLog => "Log to Log",
        }
    }
}

impl Default for ColorPass {
    fn default() -> Self {
        Self {
            color: Rgb::new(255, 0, 0),
            similarity: Param::from_value(20.0),
            reverse: false,
        }
    }
}

impl Default for ReduceInterlaceFlicker {
    fn default() -> Self {
        Self {
            softness: Param::from_value(1.0),
        }
    }
}

impl Default for CineonConverter {
    fn default() -> Self {
        Self {
            conversion: CineonConversion::default(),
            black_point_10_bit: Param::from_value(95.0),
            internal_black_point: Param::default(),
            white_point_10_bit: Param::from_value(685.0),
            internal_white_point: Param::from_value(100.0),
            gamma: Param::from_value(1.7),
            highlight_rolloff: Param::from_value(20.0),
        }
    }
}
