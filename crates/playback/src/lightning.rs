//! Lightning, drawn on the CPU since a bolt is a handful of random strokes rather than
//! a per-pixel formula.

use media::VideoFrame;

/// A straight piece of a bolt, in the picture's pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Stroke {
    pub(crate) from: [f32; 2],
    pub(crate) to: [f32; 2],
    pub(crate) width: f32,
}

/// The shape of a bolt, before it is placed in a picture.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BoltShape {
    pub(crate) segments: usize,
    /// Of the bolt's length.
    pub(crate) amplitude: f32,
    /// 0 to 1: the chance of a branch at each bend.
    pub(crate) branching: f32,
    pub(crate) width: f32,
}

/// The strokes of the bolt from `start` to `end`, the same for the same `seed`.
pub(crate) fn bolt(start: [f32; 2], end: [f32; 2], shape: BoltShape, seed: u64) -> Vec<Stroke> {
    let mut random = Random(seed);
    let mut strokes = Vec::new();
    let bends = jagged(start, end, shape.segments, shape.amplitude, &mut random);
    let length = distance(start, end);
    for (index, pair) in bends.windows(2).enumerate() {
        strokes.push(Stroke {
            from: pair[0],
            to: pair[1],
            width: shape.width,
        });
        if index == 0 || random.unit() >= shape.branching {
            continue;
        }
        let along = [pair[1][0] - pair[0][0], pair[1][1] - pair[0][1]];
        let heading = along[1].atan2(along[0]) + (random.unit() - 0.5) * 1.4;
        let reach = length * (0.2 + random.unit() * 0.2);
        let tip = [
            pair[0][0] + heading.cos() * reach,
            pair[0][1] + heading.sin() * reach,
        ];
        let twigs = jagged(
            pair[0],
            tip,
            (shape.segments / 2).max(2),
            shape.amplitude,
            &mut random,
        );
        strokes.extend(twigs.windows(2).map(|twig| Stroke {
            from: twig[0],
            to: twig[1],
            width: shape.width * 0.5,
        }));
    }
    strokes
}

/// `segments + 1` points from `start` to `end`, each bend pushed sideways at random,
/// with a smaller bend between each pair for detail.
fn jagged(
    start: [f32; 2],
    end: [f32; 2],
    segments: usize,
    amplitude: f32,
    random: &mut Random,
) -> Vec<[f32; 2]> {
    let segments = segments.max(1);
    let length = distance(start, end);
    let normal = match length > 0.0 {
        true => [(start[1] - end[1]) / length, (end[0] - start[0]) / length],
        false => [0.0, 0.0],
    };
    let reach = amplitude * length;
    let bends: Vec<[f32; 2]> = (0..=segments)
        .map(|index| {
            let t = index as f32 / segments as f32;
            let push = match index == 0 || index == segments {
                true => 0.0,
                false => (random.unit() * 2.0 - 1.0) * reach,
            };
            [
                start[0] + (end[0] - start[0]) * t + normal[0] * push,
                start[1] + (end[1] - start[1]) * t + normal[1] * push,
            ]
        })
        .collect();
    let detail = reach / segments as f32;
    let mut points = vec![start];
    for pair in bends.windows(2) {
        let push = (random.unit() * 2.0 - 1.0) * detail;
        points.push([
            (pair[0][0] + pair[1][0]) / 2.0 + normal[0] * push,
            (pair[0][1] + pair[1][1]) / 2.0 + normal[1] * push,
        ]);
        points.push(pair[1]);
    }
    points
}

/// Draws `strokes` over `frame`: `inside` along each stroke's core, `core` of its
/// width, glowing out to `outside` at its edge.
pub(crate) fn draw_strokes(
    frame: &mut VideoFrame,
    strokes: &[Stroke],
    core: f32,
    inside: [u8; 3],
    outside: [u8; 3],
) {
    let (width, height) = (frame.width as usize, frame.height as usize);
    // Coverage and how much of it is core, per pixel, so crossing strokes don't stack.
    let mut glow = vec![[0.0f32; 2]; width * height];
    for stroke in strokes {
        let half = (stroke.width / 2.0).max(0.5);
        let core_half = half * core.clamp(0.0, 1.0);
        let reach = half + 1.0;
        let left = (stroke.from[0].min(stroke.to[0]) - reach).floor().max(0.0) as usize;
        let top = (stroke.from[1].min(stroke.to[1]) - reach).floor().max(0.0) as usize;
        let right = ((stroke.from[0].max(stroke.to[0]) + reach).ceil() as usize).min(width);
        let bottom = ((stroke.from[1].max(stroke.to[1]) + reach).ceil() as usize).min(height);
        for y in top..bottom {
            for x in left..right {
                let center = [x as f32 + 0.5, y as f32 + 0.5];
                let away = to_segment(center, stroke.from, stroke.to);
                let cover = ((half - away) / (half - core_half).max(0.5)).clamp(0.0, 1.0);
                let in_core = (core_half - away + 0.5).clamp(0.0, 1.0);
                let pixel = &mut glow[y * width + x];
                pixel[0] = pixel[0].max(cover);
                pixel[1] = pixel[1].max(in_core);
            }
        }
    }
    for (pixel, [cover, in_core]) in frame.pixels.as_chunks_mut::<4>().0.iter_mut().zip(glow) {
        if cover <= 0.0 {
            continue;
        }
        let under = f32::from(pixel[3]) / 255.0 * (1.0 - cover);
        let out = cover + under;
        for channel in 0..3 {
            let lit = f32::from(outside[channel])
                + (f32::from(inside[channel]) - f32::from(outside[channel])) * in_core;
            let mixed = (lit * cover + f32::from(pixel[channel]) * under) / out;
            pixel[channel] = mixed.round() as u8;
        }
        pixel[3] = (out * 255.0).round() as u8;
    }
}

fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    (b[0] - a[0]).hypot(b[1] - a[1])
}

fn to_segment(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let along = [b[0] - a[0], b[1] - a[1]];
    let length = along[0] * along[0] + along[1] * along[1];
    let t = match length > 0.0 {
        true => (((p[0] - a[0]) * along[0] + (p[1] - a[1]) * along[1]) / length).clamp(0.0, 1.0),
        false => 0.0,
    };
    distance(p, [a[0] + along[0] * t, a[1] + along[1] * t])
}

/// SplitMix64, enough to scatter a bolt's bends.
struct Random(u64);

impl Random {
    fn unit(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 40) as f32 / (1u64 << 24) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_main_bolt_runs_from_start_to_end_and_repeats_for_its_seed() {
        let shape = BoltShape {
            segments: 7,
            amplitude: 0.1,
            branching: 0.5,
            width: 10.0,
        };
        let strokes = bolt([10.0, 20.0], [300.0, 200.0], shape, 42);
        let main: Vec<_> = strokes
            .iter()
            .filter(|stroke| stroke.width == 10.0)
            .collect();
        assert_eq!(main.len(), 14);
        assert_eq!(main[0].from, [10.0, 20.0]);
        assert_eq!(main[13].to, [300.0, 200.0]);
        assert_eq!(bolt([10.0, 20.0], [300.0, 200.0], shape, 42), strokes);
        assert_ne!(bolt([10.0, 20.0], [300.0, 200.0], shape, 43), strokes);
    }
}
