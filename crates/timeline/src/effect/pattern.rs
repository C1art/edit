//! Ellipse, Cell Pattern and Lightning: Generate effects that draw shapes and textures
//! rather than fills.

use serde::{Deserialize, Serialize};

use super::Rgb;
use crate::Param;

/// A ring shading from `inside_color` at its inner edge to `outside_color` at its
/// outer, over black or over the picture.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Ellipse {
    /// Percent of the picture's width and height.
    pub center: [Param; 2],
    /// In the picture's pixels, as is the thickness.
    pub width: Param,
    pub height: Param,
    pub thickness: Param,
    /// Percent of the thickness the ring's edges fade over.
    pub softness: Param,
    pub inside_color: Rgb,
    pub outside_color: Rgb,
    pub composite_on_original: bool,
}

/// A grey texture of cells, like bubbles or crystals, in place of the picture.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CellPattern {
    pub pattern: CellKind,
    pub invert: bool,
    /// Percent.
    pub contrast: Param,
    /// Percent: how far each cell's center strays from a regular grid.
    pub disperse: Param,
    /// Each cell's size in the picture's pixels.
    pub size: Param,
    /// In the picture's pixels.
    pub offset: [Param; 2],
    /// Degrees; the cells drift as it turns.
    pub evolution: Param,
    /// Percent.
    pub blend_with_original: Param,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CellKind {
    #[default]
    Bubbles,
    Crystals,
    Plates,
    /// Plates whose shades hold still as the pattern evolves.
    StaticPlates,
    Crystallize,
    Pillow,
}

/// A forked electrical bolt from the start point to the end point, redrawn `speed`
/// times a second.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Lightning {
    /// Percent of the picture's width and height.
    pub start_point: [Param; 2],
    pub end_point: [Param; 2],
    pub segments: Param,
    /// Percent of the bolt's length its bends reach sideways.
    pub amplitude: Param,
    /// Percent chance of a branch at each bend.
    pub branching: Param,
    /// In the picture's pixels.
    pub width: Param,
    /// Percent of the width that glows in the inside color.
    pub core_width: Param,
    pub outside_color: Rgb,
    pub inside_color: Rgb,
    /// New bolts a second; zero holds one still.
    pub speed: Param,
    pub random_seed: u32,
}

impl CellKind {
    pub const ALL: [Self; 6] = [
        Self::Bubbles,
        Self::Crystals,
        Self::Plates,
        Self::StaticPlates,
        Self::Crystallize,
        Self::Pillow,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Bubbles => "Bubbles",
            Self::Crystals => "Crystals",
            Self::Plates => "Plates",
            Self::StaticPlates => "Static Plates",
            Self::Crystallize => "Crystallize",
            Self::Pillow => "Pillow",
        }
    }
}

impl Default for Ellipse {
    fn default() -> Self {
        Self {
            center: [Param::from_value(50.0), Param::from_value(50.0)],
            width: Param::from_value(200.0),
            height: Param::from_value(200.0),
            thickness: Param::from_value(15.0),
            softness: Param::from_value(50.0),
            inside_color: Rgb::WHITE,
            outside_color: Rgb::new(0, 128, 255),
            composite_on_original: false,
        }
    }
}

impl Default for CellPattern {
    fn default() -> Self {
        Self {
            pattern: CellKind::default(),
            invert: false,
            contrast: Param::from_value(100.0),
            disperse: Param::from_value(100.0),
            size: Param::from_value(60.0),
            offset: [Param::default(), Param::default()],
            evolution: Param::default(),
            blend_with_original: Param::default(),
        }
    }
}

impl Default for Lightning {
    fn default() -> Self {
        Self {
            start_point: [Param::from_value(20.0), Param::from_value(20.0)],
            end_point: [Param::from_value(80.0), Param::from_value(80.0)],
            segments: Param::from_value(7.0),
            amplitude: Param::from_value(10.0),
            branching: Param::from_value(30.0),
            width: Param::from_value(10.0),
            core_width: Param::from_value(40.0),
            outside_color: Rgb::new(64, 0, 255),
            inside_color: Rgb::WHITE,
            speed: Param::from_value(10.0),
            random_seed: 1,
        }
    }
}
