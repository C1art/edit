//! The Generate effects, which draw over the picture.

use serde::{Deserialize, Serialize};

use super::Rgb;
use crate::Param;

/// Covers every other square of a board laid from `anchor` with `color`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Checkerboard {
    /// Percent of the picture's width and height.
    pub anchor: [Param; 2],
    /// Each square's width and height in the picture's pixels.
    pub size: [Param; 2],
    pub color: Rgb,
    /// Percent.
    pub opacity: Param,
}

/// Draws lines `border` pixels wide around cells laid from `anchor`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Grid {
    /// Percent of the picture's width and height.
    pub anchor: [Param; 2],
    /// Each cell's width and height in the picture's pixels.
    pub size: [Param; 2],
    pub border: Param,
    pub color: Rgb,
    /// Percent.
    pub opacity: Param,
    /// Fills the cells instead of the lines.
    pub invert: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Circle {
    /// Percent of the picture's width and height.
    pub center: [Param; 2],
    /// In the picture's pixels, as is the feather fading its outer edge.
    pub radius: Param,
    pub feather: Param,
    pub color: Rgb,
    /// Percent.
    pub opacity: Param,
    /// Fills outside the circle instead.
    pub invert: bool,
}

/// Fills the picture with four colors, each strongest at its point.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FourColorGradient {
    /// Percent of the picture's width and height.
    pub points: [[Param; 2]; 4],
    pub colors: [Rgb; 4],
    /// How smoothly the colors run together, from 1.
    pub blend: Param,
    /// Percent.
    pub opacity: Param,
}
