use timeline::{MaskTracking, Similarity};

/// Matches further than this many pixels from the fitted motion, or a few times the
/// typical miss, are things moving in the scene rather than the camera or the thing
/// tracked.
pub(crate) const MIN_OUTLIER_PIXELS: f64 = 1.0;
const OUTLIER_FACTOR: f64 = 2.0;
/// No hand turns or zooms this much between frames, so such a fit is wrong and the
/// matches' plain shift is used instead.
const MAX_STEP_ANGLE: f64 = 0.2;
const MAX_STEP_ZOOM: f64 = 1.2;
/// A turn or zoom is kept only where it fits the matches this much better than a plain
/// shift; small misses would otherwise read as turns that add up over a clip.
const TURN_GAIN: f64 = 0.7;

/// Where a point was in one frame and where it is in the next.
pub(crate) type Pair = ([f64; 2], [f64; 2]);

/// The similarity that best carries each pair's first point to its second, turning and
/// scaling only as `tracking` allows, refitted without the pairs it misses by much.
pub(crate) fn robust_fit(pairs: &[Pair], least_outlier: f64, tracking: MaskTracking) -> Similarity {
    let mut fit = least_squares(pairs, tracking);
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
        fit = least_squares(&inliers, tracking);
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
fn least_squares(pairs: &[Pair], tracking: MaskTracking) -> Similarity {
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
    if pairs.len() < 3 || spread < 1e-6 || !tracking.turns() {
        return shift_only;
    }
    let (a, b) = (dot / spread, cross / spread);
    let angle = b.atan2(a);
    let scale = if tracking.scales() { a.hypot(b) } else { 1.0 };
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
        let fit = robust_fit(&pairs, 1.0 / 320.0, MaskTracking::PositionScaleRotation);
        assert!((fit.shift[0] - 0.01).abs() < 1e-9 && fit.shift[1].abs() < 1e-9);
        assert!(fit.angle.abs() < 1e-9 && (fit.scale - 1.0).abs() < 1e-9);
    }
}
