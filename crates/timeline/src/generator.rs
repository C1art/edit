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
    Shape(Shape),
}

impl Generator {
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::ColorMatte { .. } => "Color Matte",
            Self::Title(_) => "Title",
            Self::AdjustmentLayer => "Adjustment Layer",
            Self::Shape(shape) => shape.kind.name(),
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
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    /// Extra space between letters in thousandths of the size, as Premiere's tracking.
    #[serde(default)]
    pub tracking: i32,
    /// Extra space between lines, in sequence pixels.
    #[serde(default)]
    pub leading: i32,
    /// An outline around the letters, outside them.
    #[serde(default)]
    pub stroke: Option<TextStroke>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TextStroke {
    pub color: Rgb,
    /// In sequence pixels.
    pub width: u32,
}

impl Default for TextStroke {
    fn default() -> Self {
        Self {
            color: Rgb::BLACK,
            width: 4,
        }
    }
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
            bold: false,
            italic: false,
            tracking: 0,
            leading: 0,
            stroke: None,
        }
    }
}

/// A shape centered on a transparent canvas the size of the sequence, as Essential
/// Graphics draws them; Motion places it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Shape {
    pub kind: ShapeKind,
    /// In sequence pixels.
    pub width: u32,
    pub height: u32,
    /// Rounds a rectangle's corners, in sequence pixels.
    #[serde(default)]
    pub corner_radius: u32,
    /// None leaves only the stroke.
    pub fill: Option<Rgb>,
    /// Drawn outside the shape, as on titles.
    #[serde(default)]
    pub stroke: Option<TextStroke>,
    #[serde(default)]
    pub shadow: bool,
}

impl Shape {
    pub fn new(kind: ShapeKind) -> Self {
        Self {
            kind,
            width: 600,
            height: 300,
            corner_radius: 0,
            fill: Some(Rgb::new(200, 60, 60)),
            stroke: None,
            shadow: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShapeKind {
    Rectangle,
    Ellipse,
}

impl ShapeKind {
    pub const ALL: [Self; 2] = [Self::Rectangle, Self::Ellipse];

    pub fn name(self) -> &'static str {
        match self {
            Self::Rectangle => "Rectangle",
            Self::Ellipse => "Ellipse",
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
