use timeline::{FrameSide, Graphic, GraphicContent, GraphicLayer, LayerPose, Pin, VerticalPin};

use super::coverage::Coverage;
use super::image_layer::draw_image;
use super::{draw_shape, draw_title, Anchor, Canvas};

/// Room kept around what a layer covers when it's cropped, so the color bled around it
/// is still there to sample when it's scaled.
const MARGIN: i64 = 2;

/// Draws `graphic`'s shown layers, bottom first, each placed as it says.
pub(super) fn draw_graphic(canvas: &mut Canvas, graphic: &Graphic) {
    for layer in graphic.layers.iter().rev().filter(|layer| !layer.hidden) {
        if let Some(drawn) = draw_layer(&layer.content, canvas.width, canvas.height) {
            canvas.cover(&drawn, &Placement::of(layer, canvas.width));
        }
    }
}

/// A layer drawn by itself on a canvas the size of the frame, cropped to what it covers.
pub(super) struct Drawn {
    canvas: Canvas,
    /// Where the crop's top left sat on the frame.
    origin: [i64; 2],
}

/// `None` when the layer covers nothing.
pub(super) fn draw_layer(content: &GraphicContent, width: u32, height: u32) -> Option<Drawn> {
    let mut drawn = Canvas::new(width, height);
    match content {
        GraphicContent::Text(title) => draw_title(&mut drawn, title, Anchor::Center),
        GraphicContent::Shape(shape) => draw_shape(&mut drawn, shape),
        GraphicContent::Image(path) => draw_image(&mut drawn, path),
    }
    drawn.cropped()
}

/// Where and how a drawn layer lands on the frame.
#[derive(Debug, Clone, Copy)]
pub(super) struct Placement {
    /// From the frame's center to the layer's, in pixels.
    offset: [f32; 2],
    scale: f32,
    /// Degrees clockwise.
    rotation: f32,
    opacity: f32,
    reveal: Option<(FrameSide, f32)>,
}

impl Placement {
    /// As `layer` is set, with its pinned edge where its position says.
    pub(super) fn of(layer: &GraphicLayer, width: u32) -> Self {
        let scale = layer.scale.max(1) as f32 / 100.0;
        let extent = match &layer.content {
            GraphicContent::Text(title) => {
                Coverage::title_extent(title, width).map_or([0.0; 2], |e| [e.x, e.y])
            }
            GraphicContent::Shape(shape) => [shape.width as f32, shape.height as f32],
            GraphicContent::Image(_) => [0.0; 2],
        };
        // Whole pixels, so unscaled text stays crisp.
        let [half_x, half_y] = extent.map(|extent| (extent * scale / 2.0).round());
        let pinned_x = match layer.pin {
            Pin::Left => half_x,
            Pin::Center => 0.0,
            Pin::Right => -half_x,
        };
        let pinned_y = match layer.pin_vertical {
            VerticalPin::Top => half_y,
            VerticalPin::Middle => 0.0,
            VerticalPin::Bottom => -half_y,
        };
        let [x, y] = layer.offset.map(|offset| offset as f32);
        Self {
            offset: [x + pinned_x, y + pinned_y],
            scale,
            rotation: layer.rotation as f32,
            opacity: layer.opacity.min(100) as f32 / 100.0,
            reveal: None,
        }
    }

    /// Moved, scaled and faded as `pose` says, on a frame `height` pixels tall.
    pub(super) fn posed(self, pose: LayerPose, height: u32) -> Self {
        let [x, y] = self.offset;
        let [dx, dy] = pose.shift.map(|shift| (shift * height as f32).round());
        Self {
            offset: [x + dx, y + dy],
            scale: self.scale * pose.scale,
            opacity: self.opacity * pose.opacity,
            reveal: pose.reveal,
            ..self
        }
    }
}

impl Canvas {
    fn cropped(self) -> Option<Drawn> {
        let (width, height) = (i64::from(self.width), i64::from(self.height));
        let (mut left, mut top, mut right, mut bottom) = (width, height, 0, 0);
        for (index, pixel) in self.pixels.iter().enumerate() {
            if pixel[3] > 0.0 {
                let (x, y) = (index as i64 % width, index as i64 / width);
                (left, top) = (left.min(x), top.min(y));
                (right, bottom) = (right.max(x + 1), bottom.max(y + 1));
            }
        }
        if left >= right {
            return None;
        }
        let (left, top) = ((left - MARGIN).max(0), (top - MARGIN).max(0));
        let (right, bottom) = ((right + MARGIN).min(width), (bottom + MARGIN).min(height));
        let mut crop = Canvas::new((right - left) as u32, (bottom - top) as u32);
        let span = (right - left) as usize;
        for (row, y) in (top..bottom).enumerate() {
            let from = (y * width + left) as usize;
            crop.pixels[row * span..(row + 1) * span]
                .copy_from_slice(&self.pixels[from..from + span]);
        }
        Some(Drawn {
            canvas: crop,
            origin: [left, top],
        })
    }

    /// Composites `top` over the canvas, scaled and turned about the canvas's center,
    /// then moved, faded and uncovered as `placement` says.
    pub(super) fn cover(&mut self, top: &Drawn, placement: &Placement) {
        let opacity = placement.opacity.clamp(0.0, 1.0);
        if opacity <= 0.0 || placement.reveal.is_some_and(|(_, shown)| shown <= 0.0) {
            return;
        }
        let (width, height) = (i64::from(self.width), i64::from(self.height));
        let [ox, oy] = top.origin;
        let (crop_width, crop_height) = (i64::from(top.canvas.width), i64::from(top.canvas.height));
        let revealed = |x: f32, y: f32| {
            let Some((side, shown)) = placement.reveal else {
                return true;
            };
            let (left, right) = (ox as f32, (ox + crop_width) as f32);
            let (upper, lower) = (oy as f32, (oy + crop_height) as f32);
            match side {
                FrameSide::Left => x < left + (right - left) * shown,
                FrameSide::Right => x >= right - (right - left) * shown,
                FrameSide::Top => y < upper + (lower - upper) * shown,
                FrameSide::Bottom => y >= lower - (lower - upper) * shown,
            }
        };
        let [dx, dy] = placement.offset;
        // Whole-pixel moves read pixels as they are, keeping text crisp.
        let exact = placement.scale == 1.0
            && placement.rotation.rem_euclid(360.0) == 0.0
            && dx.fract() == 0.0
            && dy.fract() == 0.0;
        if exact {
            let (dx, dy) = (dx as i64, dy as i64);
            for y in (oy + dy).max(0)..(oy + dy + crop_height).min(height) {
                for x in (ox + dx).max(0)..(ox + dx + crop_width).min(width) {
                    let (from_x, from_y) = (x - dx, y - dy);
                    if !revealed(from_x as f32 + 0.5, from_y as f32 + 0.5) {
                        continue;
                    }
                    let above =
                        top.canvas.pixels[((from_y - oy) * crop_width + from_x - ox) as usize];
                    blend(&mut self.pixels[(y * width + x) as usize], above, opacity);
                }
            }
            return;
        }
        let scale = placement.scale.max(0.001);
        let (sin, cos) = placement.rotation.to_radians().sin_cos();
        let half = [width as f32 / 2.0, height as f32 / 2.0];
        let center = [half[0] + dx, half[1] + dy];
        // Where the crop's corners land bounds the pixels it can reach.
        let corners = [
            (ox, oy),
            (ox + crop_width, oy),
            (ox, oy + crop_height),
            (ox + crop_width, oy + crop_height),
        ]
        .map(|(x, y)| {
            let (a, b) = (x as f32 - half[0], y as f32 - half[1]);
            [
                center[0] + scale * (cos * a - sin * b),
                center[1] + scale * (sin * a + cos * b),
            ]
        });
        let reach = |axis: usize, end: i64| {
            let low = corners
                .iter()
                .map(|c| c[axis])
                .fold(f32::INFINITY, f32::min);
            let high = corners
                .iter()
                .map(|c| c[axis])
                .fold(f32::NEG_INFINITY, f32::max);
            (low.floor() as i64 - 1).max(0)..(high.ceil() as i64 + 1).min(end)
        };
        for y in reach(1, height) {
            for x in reach(0, width) {
                let from = [x as f32 + 0.5 - center[0], y as f32 + 0.5 - center[1]];
                let turned = [cos * from[0] + sin * from[1], cos * from[1] - sin * from[0]];
                let source = [turned[0] / scale + half[0], turned[1] / scale + half[1]];
                if !revealed(source[0], source[1]) {
                    continue;
                }
                let above = top
                    .canvas
                    .sample([source[0] - 0.5 - ox as f32, source[1] - 0.5 - oy as f32]);
                blend(&mut self.pixels[(y * width + x) as usize], above, opacity);
            }
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

/// `above` faded to `opacity` over `below`, both with straight alpha.
fn blend(below: &mut [f32; 4], mut above: [f32; 4], opacity: f32) {
    above[3] *= opacity;
    let kept = below[3] * (1.0 - above[3]);
    let alpha = above[3] + kept;
    if alpha <= 0.0 {
        // Keeps the color bled around what's drawn for when it's scaled.
        if below[3] <= 0.0 {
            *below = above;
        }
        return;
    }
    let mix = |top: f32, bottom: f32| (top * above[3] + bottom * kept) / alpha;
    *below = [
        mix(above[0], below[0]),
        mix(above[1], below[1]),
        mix(above[2], below[2]),
        alpha,
    ];
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

    #[test]
    fn a_pinned_shape_keeps_its_edge_and_a_wipe_uncovers_from_its_side() {
        let red = Rgb::new(255, 0, 0);
        let mut bar = square(red, [0, 0]);
        if let GraphicContent::Shape(shape) = &mut bar.content {
            shape.width = 8;
        }
        bar.pin = Pin::Left;
        let drawn = draw_layer(&bar.content, 16, 16).unwrap();
        let half_wiped = LayerPose {
            reveal: Some((FrameSide::Left, 0.5)),
            ..LayerPose::AS_SET
        };
        let mut canvas = Canvas::new(16, 16);
        canvas.cover(&drawn, &Placement::of(&bar, 16).posed(half_wiped, 16));
        let at = |x: usize| canvas.pixels[7 * 16 + x][3];
        // Pinned left, the 8 wide bar starts at the center; half of its crop shows.
        assert_eq!(at(7), 0.0);
        assert_eq!(at(8), 1.0);
        assert_eq!(at(10), 1.0);
        assert_eq!(at(14), 0.0);
    }

    #[test]
    fn a_bottom_pinned_shape_grows_upward_from_its_position() {
        let mut bar = square(Rgb::new(255, 0, 0), [0, 0]);
        if let GraphicContent::Shape(shape) = &mut bar.content {
            shape.height = 6;
        }
        bar.pin_vertical = VerticalPin::Bottom;
        let drawn = draw_layer(&bar.content, 16, 16).unwrap();
        let mut canvas = Canvas::new(16, 16);
        canvas.cover(&drawn, &Placement::of(&bar, 16));
        let at = |y: usize| canvas.pixels[y * 16 + 7][3];
        // The 6 tall bar ends at the center row and reaches 6 rows above it.
        assert_eq!(at(1), 0.0);
        assert_eq!(at(2), 1.0);
        assert_eq!(at(7), 1.0);
        assert_eq!(at(8), 0.0);
    }
}
