mod estimate;

use std::thread;

use media::VideoFrame;

/// How far each part of one picture moves to reach the next, sampled on a grid.
pub(crate) struct Flow {
    columns: usize,
    rows: usize,
    /// Frame pixels between grid points, the first a half step in from the corner.
    step: f32,
    /// Row by row, in frame pixels.
    vectors: Vec<[f32; 2]>,
}

impl Flow {
    /// The motion from `earlier` to `later`, which must be the same size.
    pub(crate) fn between(earlier: &VideoFrame, later: &VideoFrame) -> Self {
        estimate::estimate(earlier, later)
    }

    /// The motion at frame pixel `(x, y)`, blended from the grid points around it.
    fn at(&self, x: f32, y: f32) -> [f32; 2] {
        let gx = (x / self.step - 0.5).clamp(0.0, (self.columns - 1) as f32);
        let gy = (y / self.step - 0.5).clamp(0.0, (self.rows - 1) as f32);
        let (left, top) = (gx.floor() as usize, gy.floor() as usize);
        let (right, bottom) = (
            (left + 1).min(self.columns - 1),
            (top + 1).min(self.rows - 1),
        );
        let (fx, fy) = (gx - left as f32, gy - top as f32);
        let point = |column: usize, row: usize| self.vectors[row * self.columns + column];
        [0, 1].map(|axis| {
            let upper = point(left, top)[axis] * (1.0 - fx) + point(right, top)[axis] * fx;
            let lower = point(left, bottom)[axis] * (1.0 - fx) + point(right, bottom)[axis] * fx;
            upper * (1.0 - fy) + lower * fy
        })
    }
}

/// The picture `fraction` of the way from `earlier` to `later`: each moved along
/// `flow` to where its content would be then, and the two mixed. Keeps the earlier
/// frame's time.
pub(crate) fn interpolate(
    earlier: &VideoFrame,
    later: &VideoFrame,
    flow: &Flow,
    fraction: f32,
) -> VideoFrame {
    let (width, height) = (earlier.width as usize, earlier.height as usize);
    let mut pixels = vec![0; width * height * 4];
    let row_bytes = width * 4;
    let workers = thread::available_parallelism().map_or(1, usize::from);
    let rows_each = height.div_ceil(workers).max(1);
    thread::scope(|scope| {
        for (chunk, out) in pixels.chunks_mut(rows_each * row_bytes).enumerate() {
            scope.spawn(move || {
                for (offset, row) in out.chunks_exact_mut(row_bytes).enumerate() {
                    let y = (chunk * rows_each + offset) as f32 + 0.5;
                    for (x, pixel) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                        let x = x as f32 + 0.5;
                        let [dx, dy] = flow.at(x, y);
                        let from = sample(earlier, x - dx * fraction, y - dy * fraction);
                        let ahead = 1.0 - fraction;
                        let to = sample(later, x + dx * ahead, y + dy * ahead);
                        for ((out, from), to) in pixel.iter_mut().zip(from).zip(to) {
                            *out = (from * ahead + to * fraction).round() as u8;
                        }
                    }
                }
            });
        }
    });
    VideoFrame { pixels, ..*earlier }
}

/// The color at `(x, y)` in pixel units, centers on halves, edges repeating outward.
fn sample(frame: &VideoFrame, x: f32, y: f32) -> [f32; 4] {
    let (width, height) = (frame.width as usize, frame.height as usize);
    let gx = (x - 0.5).clamp(0.0, (width - 1) as f32);
    let gy = (y - 0.5).clamp(0.0, (height - 1) as f32);
    let (left, top) = (gx.floor() as usize, gy.floor() as usize);
    let (right, bottom) = ((left + 1).min(width - 1), (top + 1).min(height - 1));
    let (fx, fy) = (gx - left as f32, gy - top as f32);
    let texel = |column: usize, row: usize| {
        let at = (row * width + column) * 4;
        let bytes = &frame.pixels[at..at + 4];
        [0, 1, 2, 3].map(|channel| f32::from(bytes[channel]))
    };
    let (a, b, c, d) = (
        texel(left, top),
        texel(right, top),
        texel(left, bottom),
        texel(right, bottom),
    );
    [0, 1, 2, 3].map(|channel| {
        let upper = a[channel] * (1.0 - fx) + b[channel] * fx;
        let lower = c[channel] * (1.0 - fx) + d[channel] * fx;
        upper * (1.0 - fy) + lower * fy
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use timeline::Ticks;

    const WIDTH: u32 = 192;
    const HEIGHT: u32 = 96;

    /// A grey picture with a square of fixed noise whose left edge is at `left`.
    fn square_at(left: u32) -> VideoFrame {
        let pixels = (0..WIDTH * HEIGHT)
            .flat_map(|at| {
                let (x, y) = (at % WIDTH, at / WIDTH);
                let inside = (left..left + 32).contains(&x) && (32..64).contains(&y);
                let value = if inside {
                    // The noise moves with the square.
                    let seed = (x - left) * 7919 + y * 104_729;
                    (seed.wrapping_mul(2_654_435_761) >> 24) as u8
                } else {
                    96
                };
                [value, value, value, 255]
            })
            .collect();
        VideoFrame {
            pts: Ticks(0),
            width: WIDTH,
            height: HEIGHT,
            pixels,
        }
    }

    fn difference(a: &VideoFrame, b: &VideoFrame) -> f64 {
        let total: u64 = a
            .pixels
            .iter()
            .zip(&b.pixels)
            .map(|(&a, &b)| u64::from(a.abs_diff(b)))
            .sum();
        total as f64 / a.pixels.len() as f64
    }

    #[test]
    fn a_moving_square_is_drawn_partway_along_its_path() {
        let (earlier, later) = (square_at(60), square_at(84));
        let flow = Flow::between(&earlier, &later);
        let middle = interpolate(&earlier, &later, &flow, 0.5);
        let expected = square_at(72);
        let flowed = difference(&middle, &expected);
        let blended = difference(&crate::frame_blend::blend(&earlier, &later, 128), &expected);
        assert!(flowed < blended / 2.0, "{flowed} vs {blended}");
    }

    #[test]
    fn still_pictures_are_left_as_they_are() {
        let frame = square_at(40);
        let flow = Flow::between(&frame, &frame);
        assert_eq!(interpolate(&frame, &frame, &flow, 0.3).pixels, frame.pixels);
    }
}
