use serde::{Deserialize, Serialize};

use crate::{Rgb, Ticks, Timebase};

/// Generated pictures have no length of their own; they hold for as long as they're
/// trimmed to, up to this many seconds.
pub const GENERATED_SECONDS: i64 = 4 * 60 * 60;
/// What an edit places of generated media when nothing is marked, as a still would.
const PLACED_SECONDS: i64 = 5;

/// Media drawn by the editor instead of read from a file.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Generator {
    ColorMatte {
        color: Rgb,
    },
    Title(Title),
    /// Draws nothing itself; its clips' looks apply to the picture of the tracks below.
    AdjustmentLayer,
}

impl Generator {
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::ColorMatte { .. } => "Color Matte",
            Self::Title(_) => "Title",
            Self::AdjustmentLayer => "Adjustment Layer",
        }
    }
}

/// Text centered on a transparent canvas the size of the sequence.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Title {
    pub text: String,
    /// Line height in sequence pixels.
    pub size: u32,
    pub color: Rgb,
    pub align: TextAlign,
    pub font: TitleFont,
    pub shadow: bool,
    /// A box behind the text, as lower thirds use.
    #[serde(default)]
    pub background: Option<Rgb>,
}

impl Default for Title {
    fn default() -> Self {
        Self {
            text: "Title".to_owned(),
            size: 96,
            color: Rgb::WHITE,
            align: TextAlign::Center,
            font: TitleFont::Sans,
            shadow: true,
            background: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TitleFont {
    Sans,
    Mono,
}

pub(crate) fn seconds(seconds: i64, timebase: Timebase) -> Ticks {
    Timebase::new(1, 1).map_or(Ticks::ZERO, |second| {
        Ticks(seconds).rescale(second, timebase)
    })
}

pub(crate) fn placed_length(timebase: Timebase) -> Ticks {
    seconds(PLACED_SECONDS, timebase)
}
