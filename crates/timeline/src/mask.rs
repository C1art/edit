use serde::{Deserialize, Serialize};

use crate::Param;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MaskShape {
    #[default]
    Ellipse,
    Rectangle,
}

impl MaskShape {
    pub const ALL: [Self; 2] = [Self::Ellipse, Self::Rectangle];

    pub fn name(self) -> &'static str {
        match self {
            Self::Ellipse => "Ellipse",
            Self::Rectangle => "Rectangle",
        }
    }
}

/// A shape that keeps the part of the picture inside it, moving with the picture.
/// Several masks on a clip add together.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct Mask {
    pub shape: MaskShape,
    /// Percent of the picture's width and height from its top left.
    pub center: [Param; 2],
    /// Percent of the picture's width and height.
    pub size: [Param; 2],
    /// Degrees clockwise.
    pub rotation: Param,
    /// Width of the soft edge, in sequence pixels, centered on the shape's outline.
    pub feather: Param,
    /// Sequence pixels the outline grows by, or shrinks by when negative.
    pub expansion: Param,
    /// Percent.
    pub opacity: Param,
    /// Keeps the picture outside the shape instead.
    pub inverted: bool,
}

impl Default for Mask {
    fn default() -> Self {
        Self::new(MaskShape::default())
    }
}

impl Mask {
    /// A shape half the picture's size, centered, with Premiere's default feather.
    pub fn new(shape: MaskShape) -> Self {
        Self {
            shape,
            center: [Param::new(5_000), Param::new(5_000)],
            size: [Param::new(5_000), Param::new(5_000)],
            rotation: Param::default(),
            feather: Param::new(1_000),
            expansion: Param::default(),
            opacity: Param::new(10_000),
            inverted: false,
        }
    }

    pub fn params_mut(&mut self) -> impl Iterator<Item = &mut Param> {
        self.center.iter_mut().chain(&mut self.size).chain([
            &mut self.rotation,
            &mut self.feather,
            &mut self.expansion,
            &mut self.opacity,
        ])
    }

    pub fn params(&self) -> impl Iterator<Item = &Param> {
        self.center.iter().chain(&self.size).chain([
            &self.rotation,
            &self.feather,
            &self.expansion,
            &self.opacity,
        ])
    }
}
