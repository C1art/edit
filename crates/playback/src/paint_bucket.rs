use std::sync::Arc;

use media::VideoFrame;
use timeline::{BucketBlend, FillSelector, PaintBucket, Ticks, VideoEffect};

/// `frame` with the clip's Paint Bucket effects filled in, in list order.
pub(crate) fn bucket_filled(
    frame: Arc<VideoFrame>,
    effects: &[VideoEffect],
    time: Ticks,
) -> Arc<VideoFrame> {
    let mut buckets = effects
        .iter()
        .filter_map(|effect| match effect {
            VideoEffect::PaintBucket(bucket) => Some(bucket),
            _ => None,
        })
        .peekable();
    if buckets.peek().is_none() {
        return frame;
    }
    let mut filled = (*frame).clone();
    for bucket in buckets {
        fill(&mut filled, bucket, time);
    }
    Arc::new(filled)
}

fn fill(frame: &mut VideoFrame, bucket: &PaintBucket, time: Ticks) {
    let value = |param: &timeline::Param| param.value_at(time);
    let (width, height) = (frame.width as usize, frame.height as usize);
    let [x, y] = bucket
        .fill_point
        .each_ref()
        .map(|param| value(param) / 100.0);
    let seed = [x * width as f32, y * height as f32];
    let inside = |at: f32, size: usize| (0.0..size as f32).contains(&at);
    let mut coverage = if inside(seed[0], width) && inside(seed[1], height) {
        let seed = seed[1] as usize * width + seed[0] as usize;
        let tolerance = value(&bucket.tolerance).clamp(0.0, 255.0) as u8;
        flood(&frame.pixels, width, seed, bucket.fill_selector, tolerance)
    } else {
        vec![0.0; width * height]
    };
    let spread = value(&bucket.spread).round() as i32;
    if spread != 0 {
        spread_by(&mut coverage, width, spread);
    }
    let feather = value(&bucket.feather).round().max(0.0) as usize;
    if feather > 0 {
        // Two box blurs come close to a Gaussian.
        for _ in 0..2 {
            box_blur(&mut coverage, width, feather);
        }
    }
    let opacity = (value(&bucket.opacity) / 100.0).clamp(0.0, 1.0);
    let color = [bucket.color.r, bucket.color.g, bucket.color.b].map(f32::from);
    for (pixel, covered) in frame.pixels.as_chunks_mut::<4>().0.iter_mut().zip(coverage) {
        *pixel = if bucket.view_threshold {
            let level = (covered * 255.0).round() as u8;
            [level, level, level, 255]
        } else {
            painted(*pixel, color, covered * opacity, bucket.blending_mode)
        };
    }
}

/// 1 where the fill reaches from `seed` across pixels like the one there, else 0.
fn flood(
    pixels: &[u8],
    width: usize,
    seed: usize,
    selector: FillSelector,
    tolerance: u8,
) -> Vec<f32> {
    let pixels = pixels.as_chunks::<4>().0;
    let start = pixels[seed];
    let near = |a: u8, b: u8| a.abs_diff(b) <= tolerance;
    let alike = |pixel: &[u8; 4]| match selector {
        FillSelector::ColorAndAlpha => pixel.iter().zip(start).all(|(&a, b)| near(a, b)),
        FillSelector::StraightColor => pixel[..3].iter().zip(start).all(|(&a, b)| near(a, b)),
        FillSelector::Transparency => pixel[3] <= tolerance,
        FillSelector::Opacity => pixel[3] >= 255 - tolerance,
    };
    let mut covered = vec![0.0; pixels.len()];
    if !alike(&start) {
        return covered;
    }
    let mut pending = vec![seed];
    while let Some(at) = pending.pop() {
        if covered[at] > 0.0 {
            continue;
        }
        let row = at - at % width;
        let reaches =
            |covered: &[f32], index: usize| covered[index] == 0.0 && alike(&pixels[index]);
        let left = (row..at)
            .rev()
            .take_while(|&index| reaches(&covered, index));
        let left = left.last().unwrap_or(at);
        let right = (at + 1..row + width).take_while(|&index| reaches(&covered, index));
        let right = right.last().unwrap_or(at);
        covered[left..=right].fill(1.0);
        let neighbours = [row.checked_sub(width), Some(row + width)];
        for next_row in neighbours.into_iter().flatten() {
            if next_row >= pixels.len() {
                continue;
            }
            let span = left - row + next_row..=right - row + next_row;
            // One seed per run of fillable pixels is enough; the run fills sideways.
            let mut in_run = false;
            for index in span {
                let fillable = reaches(&covered, index);
                if fillable && !in_run {
                    pending.push(index);
                }
                in_run = fillable;
            }
        }
    }
    covered
}

/// Grows the covered area by `pixels` each way, or shrinks it when negative.
fn spread_by(coverage: &mut [f32], width: usize, pixels: i32) {
    let reach = pixels.unsigned_abs() as usize;
    let pick = |a: f32, b: f32| if pixels > 0 { a.max(b) } else { a.min(b) };
    let height = coverage.len() / width;
    let mut line = Vec::new();
    for (stride, count, length) in [(1, height, width), (width, width, height)] {
        for start in 0..count {
            let first = if stride == 1 { start * width } else { start };
            line.clear();
            line.extend((0..length).map(|step| coverage[first + step * stride]));
            for step in 0..length {
                let span = step.saturating_sub(reach)..(step + reach + 1).min(length);
                let picked = line[span].iter().copied().reduce(pick);
                coverage[first + step * stride] = picked.unwrap_or(line[step]);
            }
        }
    }
}

fn box_blur(coverage: &mut [f32], width: usize, radius: usize) {
    let height = coverage.len() / width;
    let mut line = Vec::new();
    for (stride, count, length) in [(1, height, width), (width, width, height)] {
        for start in 0..count {
            let first = if stride == 1 { start * width } else { start };
            line.clear();
            line.extend((0..length).map(|step| coverage[first + step * stride]));
            let mut sum: f32 = line[..radius.min(length)].iter().sum();
            for step in 0..length {
                if let Some(entering) = line.get(step + radius) {
                    sum += entering;
                }
                if let Some(leaving) = step.checked_sub(radius + 1).map(|index| line[index]) {
                    sum -= leaving;
                }
                let taps = (step + radius + 1).min(length) - step.saturating_sub(radius);
                coverage[first + step * stride] = sum / taps as f32;
            }
        }
    }
}

fn painted(pixel: [u8; 4], color: [f32; 3], fill: f32, mode: BucketBlend) -> [u8; 4] {
    let source = pixel.map(f32::from);
    let picture = source[3] / 255.0;
    let (over, under, over_alpha, under_alpha) = match mode {
        BucketBlend::Normal => (color, [source[0], source[1], source[2]], fill, picture),
        BucketBlend::Behind => ([source[0], source[1], source[2]], color, picture, fill),
        BucketBlend::FillOnly => (color, color, fill, 0.0),
    };
    let alpha = over_alpha + under_alpha * (1.0 - over_alpha);
    if alpha <= 0.0 {
        return [0; 4];
    }
    let mix = |over: f32, under: f32| {
        let mixed = (over * over_alpha + under * under_alpha * (1.0 - over_alpha)) / alpha;
        mixed.round().clamp(0.0, 255.0) as u8
    };
    [
        mix(over[0], under[0]),
        mix(over[1], under[1]),
        mix(over[2], under[2]),
        (alpha * 255.0).round() as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use timeline::{Param, Rgb};

    /// Black, with a white ring around the middle of a 9-pixel square.
    fn ringed() -> VideoFrame {
        let pixels = (0..81)
            .flat_map(|index| {
                let (x, y) = (index % 9, index / 9);
                let ring = (2..=6).contains(&x)
                    && (2..=6).contains(&y)
                    && (x == 2 || x == 6 || y == 2 || y == 6);
                if ring {
                    [255; 4]
                } else {
                    [0, 0, 0, 255]
                }
            })
            .collect();
        VideoFrame {
            pts: Ticks(0),
            width: 9,
            height: 9,
            pixels,
        }
    }

    #[test]
    fn the_fill_stops_at_edges_it_cannot_cross() {
        let mut frame = ringed();
        let bucket = PaintBucket {
            color: Rgb::new(255, 0, 0),
            ..PaintBucket::default()
        };
        fill(&mut frame, &bucket, Ticks(0));
        let at = |x: usize, y: usize| &frame.pixels[(y * 9 + x) * 4..][..4];
        assert_eq!(at(4, 4), [255, 0, 0, 255]);
        assert_eq!(at(3, 5), [255, 0, 0, 255]);
        assert_eq!(at(2, 4), [255; 4]);
        assert_eq!(at(0, 0), [0, 0, 0, 255]);

        // Grown by a pixel, the fill covers the ring but stops short of the outside.
        let mut frame = ringed();
        let spread = PaintBucket {
            spread: Param::from_value(1.0),
            ..bucket
        };
        fill(&mut frame, &spread, Ticks(0));
        let at = |x: usize, y: usize| &frame.pixels[(y * 9 + x) * 4..][..4];
        assert_eq!(at(2, 4), [255, 0, 0, 255]);
        assert_eq!(at(1, 4), [0, 0, 0, 255]);
    }
}
