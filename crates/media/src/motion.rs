use std::ops::{ControlFlow, Range};
use std::path::Path;

use timeline::{Similarity, Ticks, Timebase};

use crate::block_match::{Gray, Pyramid};
use crate::{DecodedFrame, MediaError, MediaReader, Streams};

/// Wide enough to find features in, small enough to analyze quickly.
const ANALYSIS_WIDTH: u32 = 320;
/// Matches further than this many pixels from the fitted motion, or a few times the
/// typical miss, are things moving in the scene rather than the camera.
const MIN_OUTLIER_PIXELS: f64 = 1.0;
const OUTLIER_FACTOR: f64 = 2.0;
/// No hand turns or zooms this much between frames, so such a fit is wrong and the
/// matches' plain shift is used instead.
const MAX_STEP_ANGLE: f64 = 0.2;
const MAX_STEP_ZOOM: f64 = 1.2;
/// A turn or zoom is kept only where it fits the matches this much better than a plain
/// shift; small misses would otherwise read as turns that add up over a clip.
const TURN_GAIN: f64 = 0.7;

/// Where a point was in one frame and where it is in the next.
type Pair = ([f64; 2], [f64; 2]);

/// How a picture moves from frame to frame.
#[derive(Debug, Clone, PartialEq)]
pub struct Motion {
    /// The picture's size as analyzed, for its shape.
    pub size: [u32; 2],
    /// Each frame's time and where its points went from the frame before, in square
    /// units (see `Similarity`); the first frame's is the identity.
    pub steps: Vec<(Ticks, Similarity)>,
}

/// How the camera moved through `range` of `path`'s picture. `progress` hears the
/// fraction done and can stop the work. Blocks on decoding, so run it on a worker.
pub fn track_motion(
    path: &Path,
    timebase: Timebase,
    range: Range<Ticks>,
    mut progress: impl FnMut(f32) -> ControlFlow<()>,
) -> Result<Motion, MediaError> {
    let mut reader = MediaReader::open_streams(path, timebase, Streams::Video)?;
    let video = reader
        .info()
        .video
        .as_ref()
        .ok_or(MediaError::NoPlayableStream)?;
    let size = analysis_size(video.width, video.height);
    reader.set_video_size(Some(size))?;
    reader.seek(range.start)?;
    let span = (range.end - range.start).0.max(1) as f32;
    let mut previous: Option<Pyramid> = None;
    let mut steps = Vec::new();
    while let Some(frame) = reader.next_frame()? {
        let DecodedFrame::Video(frame) = frame else {
            continue;
        };
        if frame.pts >= range.end {
            break;
        }
        let pyramid = Pyramid::new(Gray::from_rgba(&frame.pixels, frame.width, frame.height));
        let step = previous
            .as_ref()
            .map_or(Similarity::IDENTITY, |previous| step(previous, &pyramid));
        steps.push((frame.pts, step));
        previous = Some(pyramid);
        let done = (frame.pts - range.start).0 as f32 / span;
        if progress(done.clamp(0.0, 1.0)).is_break() {
            return Err(MediaError::Cancelled);
        }
    }
    Ok(Motion {
        size: [size.0, size.1],
        steps,
    })
}

fn analysis_size(width: u32, height: u32) -> (u32, u32) {
    if width <= ANALYSIS_WIDTH || width == 0 {
        return (width, height);
    }
    let scaled = u64::from(height) * u64::from(ANALYSIS_WIDTH) / u64::from(width);
    // Even, as some pixel formats need.
    let height = u32::try_from(scaled / 2 * 2).unwrap_or(2).max(2);
    (ANALYSIS_WIDTH, height)
}

/// Where the points of `previous` went in `next`.
fn step(previous: &Pyramid, next: &Pyramid) -> Similarity {
    let [width, height] = previous.size().map(f64::from);
    let unit = width.max(height);
    let square = |[x, y]: [f64; 2]| {
        [
            (x + 0.5 - width / 2.0) / unit,
            (y + 0.5 - height / 2.0) / unit,
        ]
    };
    let pairs: Vec<_> = previous
        .matches(next)
        .into_iter()
        .map(|(from, to)| (square(from), square(to)))
        .collect();
    robust_fit(&pairs, MIN_OUTLIER_PIXELS / unit)
}

/// The similarity that best carries each pair's first point to its second, refitted
/// without the pairs it misses by much.
fn robust_fit(pairs: &[Pair], least_outlier: f64) -> Similarity {
    let mut fit = least_squares(pairs);
    let mut inliers = pairs.to_vec();
    for _ in 0..6 {
        let misses: Vec<f64> = pairs.iter().map(|pair| miss(&fit, pair)).collect();
        let mut sorted = misses.clone();
        sorted.sort_by(f64::total_cmp);
        let typical = sorted.get(sorted.len() / 2).copied().unwrap_or(0.0);
        let limit = (OUTLIER_FACTOR * typical).max(least_outlier);
        inliers = pairs
            .iter()
            .zip(&misses)
            .filter(|&(_, &miss)| miss <= limit)
            .map(|(pair, _)| *pair)
            .collect();
        fit = least_squares(&inliers);
    }
    let shift = mean_shift(&inliers);
    let mean_miss = |fit: &Similarity| {
        inliers.iter().map(|pair| miss(fit, pair)).sum::<f64>() / inliers.len().max(1) as f64
    };
    if mean_miss(&fit) <= TURN_GAIN * mean_miss(&shift) {
        fit
    } else {
        shift
    }
}

fn miss(fit: &Similarity, (from, to): &Pair) -> f64 {
    let [x, y] = fit.apply(*from);
    (x - to[0]).hypot(y - to[1])
}

fn means(pairs: &[Pair]) -> ([f64; 2], [f64; 2]) {
    let count = pairs.len().max(1) as f64;
    let mean = |pick: fn(&Pair) -> [f64; 2]| {
        let sum = pairs.iter().map(pick).fold([0.0, 0.0], |sum, point| {
            [sum[0] + point[0], sum[1] + point[1]]
        });
        sum.map(|sum| sum / count)
    };
    (mean(|pair| pair.0), mean(|pair| pair.1))
}

fn mean_shift(pairs: &[Pair]) -> Similarity {
    let (from, to) = means(pairs);
    Similarity {
        shift: [to[0] - from[0], to[1] - from[1]],
        ..Similarity::IDENTITY
    }
}

/// The similarity minimizing the squared misses, or the mean shift where there are too
/// few pairs, or they're too bunched, to tell a turn or zoom.
fn least_squares(pairs: &[Pair]) -> Similarity {
    if pairs.is_empty() {
        return Similarity::IDENTITY;
    }
    let (from, to) = means(pairs);
    let shift_only = mean_shift(pairs);
    let (mut spread, mut dot, mut cross) = (0.0, 0.0, 0.0);
    for (p, q) in pairs {
        let p = [p[0] - from[0], p[1] - from[1]];
        let q = [q[0] - to[0], q[1] - to[1]];
        spread += p[0] * p[0] + p[1] * p[1];
        dot += p[0] * q[0] + p[1] * q[1];
        cross += p[0] * q[1] - p[1] * q[0];
    }
    if pairs.len() < 3 || spread < 1e-6 {
        return shift_only;
    }
    let (a, b) = (dot / spread, cross / spread);
    let (angle, scale) = (b.atan2(a), a.hypot(b));
    if angle.abs() > MAX_STEP_ANGLE || !(1.0 / MAX_STEP_ZOOM..=MAX_STEP_ZOOM).contains(&scale) {
        return shift_only;
    }
    let turn = Similarity {
        shift: [0.0, 0.0],
        angle,
        scale,
    };
    let turned = turn.apply(from);
    Similarity {
        shift: [to[0] - turned[0], to[1] - turned[1]],
        ..turn
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smooth texture at any point, so a moved picture can be drawn exactly.
    fn texture(x: f64, y: f64) -> f64 {
        let hash = |i: i64, j: i64| {
            let n = (i.wrapping_mul(73_856_093) ^ j.wrapping_mul(19_349_663)) as u64;
            (n.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 40) as f64 / (1u64 << 24) as f64
        };
        let noise = |x: f64, y: f64| {
            let (i, j) = (x.floor(), y.floor());
            let (fx, fy) = (x - i, y - j);
            let (i, j) = (i as i64, j as i64);
            let ease = |t: f64| t * t * (3.0 - 2.0 * t);
            let (sx, sy) = (ease(fx), ease(fy));
            let top = hash(i, j) * (1.0 - sx) + hash(i + 1, j) * sx;
            let bottom = hash(i, j + 1) * (1.0 - sx) + hash(i + 1, j + 1) * sx;
            top * (1.0 - sy) + bottom * sy
        };
        255.0 * (0.6 * noise(x / 6.0, y / 6.0) + 0.4 * noise(x / 2.5, y / 2.5))
    }

    /// The texture as seen after `motion` carried its points.
    fn picture(motion: &Similarity) -> Pyramid {
        let (width, height) = (320u32, 180u32);
        let unit = f64::from(width);
        let back = motion.inverse();
        let rgba: Vec<u8> = (0..height)
            .flat_map(|y| (0..width).map(move |x| (x, y)))
            .flat_map(|(x, y)| {
                let point = [
                    (f64::from(x) + 0.5) / unit - 0.5,
                    (f64::from(y) + 0.5 - f64::from(height) / 2.0) / unit,
                ];
                let [x, y] = back.apply(point);
                let value = texture(x * unit, y * unit).round() as u8;
                [value, value, value, 255]
            })
            .collect();
        Pyramid::new(Gray::from_rgba(&rgba, width, height))
    }

    #[test]
    fn a_shift_turn_and_zoom_between_frames_is_recovered() {
        let moved = Similarity {
            shift: [0.03, -0.015],
            angle: 0.02,
            scale: 1.015,
        };
        let found = step(&picture(&Similarity::IDENTITY), &picture(&moved));
        let off = (found.shift[0] - moved.shift[0]).abs() + (found.shift[1] - moved.shift[1]).abs();
        assert!(off < 1e-3, "{found:?}");
        assert!((found.angle - moved.angle).abs() < 2e-3, "{found:?}");
        assert!((found.scale - moved.scale).abs() < 2e-3, "{found:?}");
    }

    #[test]
    fn matches_on_something_moving_across_the_scene_are_ignored() {
        let shift = Similarity {
            shift: [0.01, 0.0],
            ..Similarity::IDENTITY
        };
        let mut pairs: Vec<_> = (0..40)
            .map(|k| {
                let from = [f64::from(k % 8) * 0.1 - 0.35, f64::from(k / 8) * 0.1 - 0.2];
                (from, shift.apply(from))
            })
            .collect();
        // A car crossing a corner of the frame.
        pairs.extend((0..6).map(|k| ([0.3 + f64::from(k) * 0.01, 0.2], [0.4, 0.25])));
        let fit = robust_fit(&pairs, 1.0 / 320.0);
        assert!((fit.shift[0] - 0.01).abs() < 1e-9 && fit.shift[1].abs() < 1e-9);
        assert!(fit.angle.abs() < 1e-9 && (fit.scale - 1.0).abs() < 1e-9);
    }
}
