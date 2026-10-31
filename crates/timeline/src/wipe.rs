use serde::{Deserialize, Serialize};

use crate::{Rgb, Side};

/// The shape a wipe uncovers a picture in, as it enters from the left; other sides turn
/// it. The renderer numbers patterns in declaration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WipePattern {
    /// A straight edge sweeping across.
    Edge,
    /// Opening out from a line through the middle.
    Split,
    Box,
    Round,
    Diamond,
    Cross,
    /// Sweeping around the center from twelve o'clock.
    Clock,
    /// Opening both ways around the center from twelve o'clock.
    Wedge,
    /// Sweeping from the top edge around the top left corner.
    Radial,
    /// Eight blades sweeping around the center at once.
    Pinwheel,
    /// Bands that each wipe across at once.
    Blinds,
    /// Alternate squares of a checkerboard wipe across, then the others.
    Checker,
    /// A box growing from the top left corner.
    Inset,
    /// Blocks appearing in a fixed random order.
    Blocks,
    /// Bands across the picture, each wiping in from the opposite side to the last.
    Bands,
    /// Rows of squares, each row half a square along from the last, all wiping across
    /// at once.
    Staggered,
    /// Boxes one at a time in a spiral from the top left corner in to the middle.
    Spiral,
    /// Boxes one at a time up the first column, down the next, and so on across.
    ZigZag,
    /// Blocks in a random order that sweeps across.
    Scatter,
    /// Blotches that spread until they meet.
    Splatter,
    /// Through a soft cloudy gradient, darkest first, drifting from the entry side.
    Gradient,
}

/// Whether a wipe uncovers the incoming picture as its pattern grows, or as it shrinks:
/// a reversed iris closes in on the outgoing picture.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Sweep {
    #[default]
    Forward,
    Reverse,
}

impl Sweep {
    pub(crate) fn is_forward(&self) -> bool {
        *self == Self::Forward
    }
}

/// A band drawn along a wipe's edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Border {
    /// Sequence pixels.
    pub width: u16,
    pub color: Rgb,
}

impl Border {
    pub const NONE: Self = Self {
        width: 0,
        color: Rgb::WHITE,
    };

    /// Only the default is left out of a saved file, so a colour picked before the
    /// width is kept.
    pub(crate) fn is_none(&self) -> bool {
        *self == Self::NONE
    }
}

impl Default for Border {
    fn default() -> Self {
        Self::NONE
    }
}

/// A transition's pattern showing part of a picture.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wipe {
    pub pattern: WipePattern,
    pub from: Side,
    /// 0 hides the picture and 1 shows all of it.
    pub progress: f32,
    pub sweep: Sweep,
    pub border: Border,
    /// How much of the pattern's order the edge blurs across, from 0 to 1.
    pub softness: f32,
}

impl Wipe {
    pub fn is_visible(&self) -> bool {
        self.progress > 0.0
    }
}
