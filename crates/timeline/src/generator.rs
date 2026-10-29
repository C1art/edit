use std::f64::consts::SQRT_2;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{CountingLeader, Graphic, Rgb, ShadowStyle, Ticks, Timebase, TitleMotion};

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
    /// SMPTE color bars, with a 1 kHz reference tone at -20 dBFS.
    BarsAndTone,
    BlackVideo,
    /// Draws nothing itself, for effects that draw, such as Timecode, to draw on.
    TransparentVideo,
    /// Lasts its own eight seconds, not as long as it's trimmed to.
    CountingLeader(CountingLeader),
    Graphic(Graphic),
}

impl Generator {
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::ColorMatte { .. } => "Color Matte",
            Self::Title(_) => "Title",
            Self::AdjustmentLayer => "Adjustment Layer",
            Self::Shape(shape) => shape.kind.name(),
            Self::BarsAndTone => "Bars and Tone",
            Self::BlackVideo => "Black Video",
            Self::TransparentVideo => "Transparent Video",
            Self::CountingLeader(_) => "Universal Counting Leader",
            Self::Graphic(_) => "Graphic",
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
    /// The shadow's look, scaled to the text when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow_style: Option<ShadowStyle>,
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
    #[serde(default)]
    pub motion: TitleMotion,
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

impl Title {
    pub fn shadow_look(&self) -> ShadowStyle {
        self.shadow_style.unwrap_or_else(|| self.usual_shadow())
    }

    /// The shadow unless one is set: offset a 24th of the size each way.
    pub fn usual_shadow(&self) -> ShadowStyle {
        let offset = f64::from((self.size as f32 / 24.0).round().max(1.0));
        ShadowStyle::falling((offset * SQRT_2).round() as u32, self.size / 32)
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
            shadow_style: None,
            background: None,
            bold: false,
            italic: false,
            tracking: 0,
            leading: 0,
            stroke: None,
            motion: TitleMotion::Still,
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
    /// The shadow's look, scaled to the frame when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow_style: Option<ShadowStyle>,
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
            shadow_style: None,
        }
    }

    /// The shadow as drawn on a frame `height` pixels tall.
    pub fn shadow_look(&self, height: u32) -> ShadowStyle {
        self.shadow_style
            .unwrap_or_else(|| Self::usual_shadow(height))
    }

    /// The shadow unless one is set, scaled to the frame: a 1080-line frame offsets it 9
    /// pixels each way.
    pub fn usual_shadow(height: u32) -> ShadowStyle {
        let offset = f64::from((height / 120).max(1));
        ShadowStyle::falling((offset * SQRT_2).round() as u32, height / 160)
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TitleFont {
    Sans,
    Mono,
    /// A TrueType or OpenType file, as installed fonts are.
    File(PathBuf),
}

impl TitleFont {
    pub fn name(&self) -> String {
        match self {
            Self::Sans => "Sans".into(),
            Self::Mono => "Mono".into(),
            Self::File(path) => path.file_stem().map_or_else(
                || path.display().to_string(),
                |stem| stem.to_string_lossy().into_owned(),
            ),
        }
    }
}

pub(crate) fn seconds(seconds: i64, timebase: Timebase) -> Ticks {
    Timebase::new(1, 1).map_or(Ticks::ZERO, |second| {
        Ticks(seconds).rescale(second, timebase)
    })
}

/// How long generated media and still pictures last in the pool, which clips of them
/// can be trimmed anywhere within.
pub fn held_length(timebase: Timebase) -> Ticks {
    seconds(GENERATED_SECONDS, timebase)
}

pub(crate) fn placed_length(timebase: Timebase) -> Ticks {
    seconds(PLACED_SECONDS, timebase)
}
