use timeline::{Graphic, GraphicContent, GraphicLayer};

use super::{draw_shape, draw_title, Anchor, Canvas};

/// Draws `graphic`'s shown layers, bottom first, each placed as it says.
pub(super) fn draw_graphic(canvas: &mut Canvas, graphic: &Graphic) {
    for layer in graphic.layers.iter().rev().filter(|layer| !layer.hidden) {
        let mut drawn = Canvas::new(canvas.width, canvas.height);
        match &layer.content {
            GraphicContent::Text(title) => draw_title(&mut drawn, title, Anchor::Center),
            GraphicContent::Shape(shape) => draw_shape(&mut drawn, shape),
        }
        canvas.cover(&drawn, layer);
    }
}

impl Canvas {
    /// Composites `top` over the canvas, scaled and turned about the canvas's center,
    /// then moved by `layer`'s offset and faded to its opacity.
    fn cover(&mut self, top: &Canvas, layer: &GraphicLayer) {
        let (width, height) = (i64::from(self.width), i64::from(self.height));
        let [dx, dy] = layer.offset.map(i64::from);
        let opacity = (layer.opacity.min(100) as f32) / 100.0;
        // Whole-pixel moves read pixels as they are, keeping text crisp.
        let exact = layer.scale == 100 && layer.rotation.rem_euclid(360) == 0;
        let scale = (layer.scale.max(1) as f32) / 100.0;
        let (sin, cos) = (layer.rotation as f32).to_radians().sin_cos();
        let center = [
            width as f32 / 2.0 + dx as f32,
            height as f32 / 2.0 + dy as f32,
        ];
        for (index, below) in self.pixels.iter_mut().enumerate() {
            let (x, y) = (index as i64 % width, index as i64 / width);
            let mut above = if exact {
                let (x, y) = (x - dx, y - dy);
                if !(0..width).contains(&x) || !(0..height).contains(&y) {
                    continue;
                }
                top.pixels[(y * width + x) as usize]
            } else {
                let from = [x as f32 + 0.5 - center[0], y as f32 + 0.5 - center[1]];
                let turned = [cos * from[0] + sin * from[1], cos * from[1] - sin * from[0]];
                top.sample([
                    turned[0] / scale + width as f32 / 2.0 - 0.5,
                    turned[1] / scale + height as f32 / 2.0 - 0.5,
                ])
            };
            above[3] *= opacity;
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

    /// The color between pixels at `[x, y]`, in pixels from the first one's center,
    /// blended by coverage so the color bled around shapes doesn't fringe them.
    fn sample(&self, [x, y]: [f32; 2]) -> [f32; 4] {
        let (left, top) = (x.floor(), y.floor());
        let (right_share, bottom_share) = (x - left, y - top);
        let mut sum = [0.0; 4];
        for (column, row, weight) in [
            (0, 0, (1.0 - right_share) * (1.0 - bottom_share)),
            (1, 0, right_share * (1.0 - bottom_share)),
            (0, 1, (1.0 - right_share) * bottom_share),
            (1, 1, right_share * bottom_share),
        ] {
            let (x, y) = (left as i64 + column, top as i64 + row);
            if !(0..i64::from(self.width)).contains(&x) || !(0..i64::from(self.height)).contains(&y)
            {
                continue;
            }
            let pixel = self.pixels[(y * i64::from(self.width) + x) as usize];
            let covered = weight * pixel[3];
            sum = [
                sum[0] + pixel[0] * covered,
                sum[1] + pixel[1] * covered,
                sum[2] + pixel[2] * covered,
                sum[3] + covered,
            ];
        }
        match sum[3] > 0.0 {
            true => [sum[0] / sum[3], sum[1] / sum[3], sum[2] / sum[3], sum[3]],
            false => [0.0; 4],
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

    #[test]
    fn layers_scale_turn_and_fade_about_their_center() {
        let draw = |layer: GraphicLayer| {
            let mut canvas = Canvas::new(16, 16);
            let graphic = Graphic {
                layers: vec![layer],
            };
            draw_graphic(&mut canvas, &graphic);
            move |x: usize, y: usize| canvas.pixels[y * 16 + x][3]
        };
        let red = Rgb::new(255, 0, 0);
        // A 4 by 2 bar, turned upright, covers columns 7 and 8 of rows 6 to 9.
        let mut bar = square(red, [0, 0]);
        if let GraphicContent::Shape(shape) = &mut bar.content {
            shape.height = 2;
        }
        let turned = draw(GraphicLayer {
            rotation: 90,
            ..bar
        });
        assert!(turned(7, 6) > 0.99);
        assert!(turned(6, 7) < 0.01);
        // Doubled, the square covers columns 4 to 11.
        let doubled = draw(GraphicLayer {
            scale: 200,
            ..square(red, [0, 0])
        });
        assert!(doubled(5, 5) > 0.99);
        assert_eq!(doubled(2, 2), 0.0);
        let faded = draw(GraphicLayer {
            opacity: 50,
            ..square(red, [0, 0])
        });
        assert_eq!(faded(7, 7), 0.5);
    }
}
