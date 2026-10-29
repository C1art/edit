use timeline::{Graphic, GraphicContent};

use super::{draw_shape, draw_title, Anchor, Canvas};

/// Draws `graphic`'s shown layers, bottom first, each moved by its offset.
pub(super) fn draw_graphic(canvas: &mut Canvas, graphic: &Graphic) {
    for layer in graphic.layers.iter().rev().filter(|layer| !layer.hidden) {
        let mut drawn = Canvas::new(canvas.width, canvas.height);
        match &layer.content {
            GraphicContent::Text(title) => draw_title(&mut drawn, title, Anchor::Center),
            GraphicContent::Shape(shape) => draw_shape(&mut drawn, shape),
        }
        canvas.cover(&drawn, layer.offset);
    }
}

impl Canvas {
    /// Composites `top`, moved `offset` pixels, over the canvas.
    fn cover(&mut self, top: &Canvas, [dx, dy]: [i32; 2]) {
        let (width, height) = (i64::from(self.width), i64::from(self.height));
        for (index, below) in self.pixels.iter_mut().enumerate() {
            let index = index as i64;
            let (x, y) = (index % width - i64::from(dx), index / width - i64::from(dy));
            if !(0..width).contains(&x) || !(0..height).contains(&y) {
                continue;
            }
            let above = top.pixels[(y * width + x) as usize];
            let kept = below[3] * (1.0 - above[3]);
            let alpha = above[3] + kept;
            if alpha <= 0.0 {
                // Keeps the color bled around what's drawn for when it's scaled.
                if below[3] <= 0.0 {
                    *below = above;
                }
                continue;
            }
            let mix = |top: f32, bottom: f32| (top * above[3] + bottom * kept) / alpha;
            *below = [
                mix(above[0], below[0]),
                mix(above[1], below[1]),
                mix(above[2], below[2]),
                alpha,
            ];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use timeline::{GraphicLayer, Rgb, Shape, ShapeKind};

    fn square(color: Rgb, offset: [i32; 2]) -> GraphicLayer {
        let mut shape = Shape::new(ShapeKind::Rectangle);
        (shape.width, shape.height, shape.fill) = (4, 4, Some(color));
        GraphicLayer {
            offset,
            ..GraphicLayer::new(GraphicContent::Shape(shape))
        }
    }

    #[test]
    fn layers_draw_moved_with_the_first_on_top() {
        let red = Rgb::new(255, 0, 0);
        let blue = Rgb::new(0, 0, 255);
        let graphic = Graphic {
            layers: vec![square(red, [0, 0]), square(blue, [4, 0])],
        };
        let mut canvas = Canvas::new(16, 16);
        draw_graphic(&mut canvas, &graphic);
        let at = |x: usize, y: usize| canvas.pixels[y * 16 + x];
        assert_eq!(at(7, 7), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(at(11, 7), [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(at(2, 7)[3], 0.0);
    }
}
