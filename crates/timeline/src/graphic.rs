//! Graphics with several layers, as Essential Graphics builds them: text and shapes
//! drawn one over another in a single clip.

use serde::{Deserialize, Serialize};

use crate::{Generator, Shape, Title, TitleMotion};

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
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GraphicContent {
    /// Still text; rolls and crawls belong to a whole title.
    Text(Title),
    Shape(Shape),
}

impl GraphicLayer {
    pub fn new(content: GraphicContent) -> Self {
        let name = match &content {
            GraphicContent::Text(title) => title.text.lines().next().unwrap_or_default(),
            GraphicContent::Shape(shape) => shape.kind.name(),
        };
        Self {
            name: name.to_owned(),
            content,
            offset: [0, 0],
            hidden: false,
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
