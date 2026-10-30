//! Graphics with several layers, as Essential Graphics builds them: text, shapes and
//! pictures drawn one over another in a single clip.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{Generator, LayerAnimation, Shape, Title, TitleMotion};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Graphic {
    /// Topmost first, as the Essential Graphics panel lists them.
    pub layers: Vec<GraphicLayer>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GraphicLayer {
    pub name: String,
    pub content: GraphicContent,
    /// From the frame's center to the layer's, in sequence pixels, down positive.
    #[serde(default)]
    pub offset: [i32; 2],
    #[serde(default)]
    pub hidden: bool,
    /// Percent of the size it's drawn at, about its center.
    #[serde(default = "full")]
    pub scale: u32,
    /// Degrees clockwise about its center.
    #[serde(default)]
    pub rotation: i32,
    /// Percent.
    #[serde(default = "full")]
    pub opacity: u32,
    #[serde(default)]
    pub pin: Pin,
    #[serde(default)]
    pub pin_vertical: VerticalPin,
    #[serde(default)]
    pub animation: LayerAnimation,
}

/// Which edge of a layer stays at its position as what's in it grows, so text typed
/// into a left-aligned lower third grows to the right.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Pin {
    Left,
    #[default]
    Center,
    Right,
}

impl Pin {
    pub const ALL: [Self; 3] = [Self::Left, Self::Center, Self::Right];

    pub fn name(self) -> &'static str {
        match self {
            Self::Left => "Left",
            Self::Center => "Center",
            Self::Right => "Right",
        }
    }
}

/// Which edge stays put as a layer grows taller, so a lower third pinned to its bottom
/// grows upward as lines are added.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum VerticalPin {
    Top,
    #[default]
    Middle,
    Bottom,
}

impl VerticalPin {
    pub const ALL: [Self; 3] = [Self::Top, Self::Middle, Self::Bottom];

    pub fn name(self) -> &'static str {
        match self {
            Self::Top => "Top",
            Self::Middle => "Middle",
            Self::Bottom => "Bottom",
        }
    }
}

fn full() -> u32 {
    100
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GraphicContent {
    /// Still text; rolls and crawls belong to a whole title.
    Text(Title),
    Shape(Shape),
    /// A still picture from a file, such as a logo, at its own size or shrunk to fit
    /// the frame.
    Image(PathBuf),
}

impl GraphicLayer {
    pub fn new(content: GraphicContent) -> Self {
        let name = match &content {
            GraphicContent::Text(title) => title.text.lines().next().unwrap_or_default().to_owned(),
            GraphicContent::Shape(shape) => shape.kind.name().to_owned(),
            GraphicContent::Image(path) => path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default(),
        };
        Self {
            name,
            content,
            offset: [0, 0],
            hidden: false,
            scale: full(),
            rotation: 0,
            opacity: full(),
            pin: Pin::Center,
            pin_vertical: VerticalPin::Middle,
            animation: LayerAnimation::default(),
        }
    }
}

impl Graphic {
    /// `generator` as the bottom layer of a graphic, for a still title or a shape.
    pub fn layered(generator: &Generator) -> Option<Self> {
        let content = match generator {
            Generator::Title(title) if title.motion == TitleMotion::Still => {
                GraphicContent::Text(title.clone())
            }
            Generator::Shape(shape) => GraphicContent::Shape(*shape),
            _ => return None,
        };
        Some(Self {
            layers: vec![GraphicLayer::new(content)],
        })
    }

    /// Moves the layer at `index` one place toward the top, or the bottom when `down`;
    /// where it now is, if it moved.
    pub fn restack(&mut self, index: usize, down: bool) -> Option<usize> {
        let to = if down {
            index.checked_add(1).filter(|&to| to < self.layers.len())?
        } else {
            index.checked_sub(1)?
        };
        self.layers.get(index)?;
        self.layers.swap(index, to);
        Some(to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ShapeKind;

    #[test]
    fn restacking_stops_at_the_top_and_bottom() {
        let shape = |kind| GraphicLayer::new(GraphicContent::Shape(Shape::new(kind)));
        let mut graphic = Graphic {
            layers: vec![shape(ShapeKind::Rectangle), shape(ShapeKind::Ellipse)],
        };
        assert_eq!(graphic.restack(0, false), None);
        assert_eq!(graphic.restack(1, true), None);
        assert_eq!(graphic.restack(0, true), Some(1));
        assert_eq!(graphic.layers[0].name, "Ellipse");
        assert_eq!(graphic.restack(5, false), None);
    }
}
