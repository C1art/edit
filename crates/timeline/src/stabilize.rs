use serde::{Deserialize, Serialize};

use crate::{Ticks, Timebase};

/// The most Auto-scale enlarges the picture to hide the edges the correction reveals,
/// as Premiere's default Maximum Scale.
const MAX_ZOOM: f64 = 1.5;
/// At 100% smoothness the camera path is averaged over about this many seconds each way.
const SMOOTHEST_SECONDS: f64 = 2.0;

/// Scales, turns and then shifts points in square units: shares of the picture's longer
/// side, measured from its center with y down.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Similarity {
    pub shift: [f64; 2],
    /// Radians, clockwise on screen since y points down.
    pub angle: f64,
    pub scale: f64,
}

impl Similarity {
    pub const IDENTITY: Self = Self {
        shift: [0.0, 0.0],
        angle: 0.0,
        scale: 1.0,
    };

    pub fn apply(&self, [x, y]: [f64; 2]) -> [f64; 2] {
        let (sin, cos) = self.angle.sin_cos();
        [
            self.scale * (cos * x - sin * y) + self.shift[0],
            self.scale * (sin * x + cos * y) + self.shift[1],
        ]
    }

    /// This one, then `next`.
    pub fn then(&self, next: &Self) -> Self {
        Self {
            shift: next.apply(self.shift),
            angle: self.angle + next.angle,
            scale: self.scale * next.scale,
        }
    }

    pub fn inverse(&self) -> Self {
        let turn = Self {
            shift: [0.0, 0.0],
            angle: -self.angle,
            scale: 1.0 / self.scale,
        };
        let [x, y] = turn.apply(self.shift);
        Self {
            shift: [-x, -y],
            ..turn
        }
    }
}

/// A similarity stored exactly: millionths of square units, microradians, and
/// millionths of the scale's natural log, which averages evenly.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "[i32; 4]", into = "[i32; 4]")]
pub struct Pose {
    pub x: i32,
    pub y: i32,
    pub angle: i32,
    pub zoom: i32,
}

impl From<[i32; 4]> for Pose {
    fn from([x, y, angle, zoom]: [i32; 4]) -> Self {
        Self { x, y, angle, zoom }
    }
}

impl From<Pose> for [i32; 4] {
    fn from(pose: Pose) -> Self {
        [pose.x, pose.y, pose.angle, pose.zoom]
    }
}

impl Pose {
    fn values(self) -> [f64; 4] {
        <[i32; 4]>::from(self).map(|value| f64::from(value) / 1e6)
    }

    fn from_values(values: [f64; 4]) -> Self {
        // Saturating keeps a runaway estimate from wrapping; no real camera gets near.
        Self::from(values.map(|value| (value * 1e6).round() as i32))
    }
}

impl From<Similarity> for Pose {
    fn from(motion: Similarity) -> Self {
        let [x, y] = motion.shift;
        Self::from_values([x, y, motion.angle, motion.scale.max(1e-6).ln()])
    }
}

impl From<Pose> for Similarity {
    fn from(pose: Pose) -> Self {
        let [x, y, angle, zoom] = pose.values();
        Self {
            shift: [x, y],
            angle,
            scale: zoom.exp(),
        }
    }
}

/// How Warp Stabilizer treats the camera's motion.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StabilizeMethod {
    /// Keeps the camera's intended moves, smoothed.
    #[default]
    SmoothMotion,
    /// Holds the camera still for the whole clip.
    NoMotion,
}

impl StabilizeMethod {
    pub const ALL: [Self; 2] = [Self::SmoothMotion, Self::NoMotion];

    pub fn name(self) -> &'static str {
        match self {
            Self::SmoothMotion => "Smooth Motion",
            Self::NoMotion => "No Motion",
        }
    }
}

/// What Warp Stabilizer does about the edges moving the picture reveals.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Framing {
    StabilizeOnly,
    /// Enlarges the picture just enough that its edges stay outside the frame.
    #[default]
    CropAutoScale,
}

impl Framing {
    pub const ALL: [Self; 2] = [Self::StabilizeOnly, Self::CropAutoScale];

    pub fn name(self) -> &'static str {
        match self {
            Self::StabilizeOnly => "Stabilize Only",
            Self::CropAutoScale => "Stabilize, Crop, Auto-scale",
        }
    }
}

/// The camera's path through a clip's source, as analysis found it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CameraPath {
    /// Each analyzed frame's source time, in order.
    pub frames: Vec<Ticks>,
    /// Where each frame's points are, from where they were in the first frame.
    pub poses: Vec<Pose>,
    /// The picture's size as analyzed, for its shape.
    pub size: [u32; 2],
}

/// Warp Stabilizer's settings and the correction they make of the analyzed path.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Stabilizer {
    /// Percent.
    pub smoothness: u32,
    pub method: StabilizeMethod,
    pub framing: Framing,
    #[serde(default)]
    pub path: Option<CameraPath>,
    /// For each analyzed frame, where each point of the stabilized picture is found in
    /// the source's. Kept up to date with the settings by `retune`.
    #[serde(default)]
    pub corrections: Vec<Pose>,
    /// How much Auto-scale enlarges the picture, in thousandths of its size.
    #[serde(default = "unscaled")]
    pub auto_scale: u32,
}

fn unscaled() -> u32 {
    1000
}

impl Default for Stabilizer {
    fn default() -> Self {
        Self {
            smoothness: 50,
            method: StabilizeMethod::default(),
            framing: Framing::default(),
            path: None,
            corrections: Vec::new(),
            auto_scale: unscaled(),
        }
    }
}

impl Stabilizer {
    /// Takes the analysis of a clip: its picture's `size` and each frame's motion from
    /// the frame before, which the first frame's is ignored.
    pub fn analyzed(&mut self, size: [u32; 2], steps: &[(Ticks, Similarity)], timebase: Timebase) {
        let mut camera = Similarity::IDENTITY;
        let poses = steps
            .iter()
            .enumerate()
            .map(|(index, (_, step))| {
                if index > 0 {
                    camera = camera.then(step);
                }
                Pose::from(camera)
            })
            .collect();
        self.path = Some(CameraPath {
            frames: steps.iter().map(|&(time, _)| time).collect(),
            poses,
            size,
        });
        self.retune(timebase);
    }

    /// Recomputes the corrections after the settings change.
    pub fn retune(&mut self, timebase: Timebase) {
        let Some(path) = &self.path else {
            self.corrections.clear();
            self.auto_scale = unscaled();
            return;
        };
        let camera: Vec<[f64; 4]> = path.poses.iter().map(|pose| pose.values()).collect();
        let steady = match self.method {
            StabilizeMethod::SmoothMotion => {
                let sigma = frames_per_second(&path.frames, timebase)
                    * SMOOTHEST_SECONDS
                    * f64::from(self.smoothness.min(100))
                    / 100.0;
                smooth(&camera, sigma)
            }
            StabilizeMethod::NoMotion => vec![mean(&camera); camera.len()],
        };
        let corrections: Vec<Similarity> = camera
            .iter()
            .zip(&steady)
            .map(|(&camera, &steady)| {
                let [camera, steady] =
                    [camera, steady].map(|values| Similarity::from(Pose::from_values(values)));
                steady.inverse().then(&camera)
            })
            .collect();
        let zoom = match self.framing {
            Framing::StabilizeOnly => 1.0,
            Framing::CropAutoScale => {
                let aspect = aspect(path.size);
                corrections
                    .iter()
                    .map(|correction| covering_zoom(correction, aspect))
                    .fold(1.0, f64::max)
            }
        };
        let enlarge = Similarity {
            scale: 1.0 / zoom,
            ..Similarity::IDENTITY
        };
        self.auto_scale = (zoom * 1000.0).round() as u32;
        self.corrections = corrections
            .iter()
            .map(|correction| Pose::from(enlarge.then(correction)))
            .collect();
    }

    /// Where each point of the stabilized picture at source time `time` is in the
    /// source's, if the clip has been analyzed.
    pub fn correction_at(&self, time: Ticks) -> Option<Similarity> {
        let path = self.path.as_ref()?;
        let index = path.frames.partition_point(|&frame| frame <= time);
        let correction = self.corrections.get(index.saturating_sub(1))?;
        Some(Similarity::from(*correction))
    }
}

fn aspect([width, height]: [u32; 2]) -> [f64; 2] {
    let longer = f64::from(width.max(height).max(1));
    [f64::from(width) / longer, f64::from(height) / longer]
}

pub(crate) fn frames_per_second(frames: &[Ticks], timebase: Timebase) -> f64 {
    let (Some(first), Some(last)) = (frames.first(), frames.last()) else {
        return 0.0;
    };
    let span = (last.0 - first.0) as f64 * f64::from(timebase.num()) / f64::from(timebase.den());
    if span > 0.0 {
        (frames.len() - 1) as f64 / span
    } else {
        0.0
    }
}

/// Each value averaged with its neighbors by a Gaussian of `sigma` frames; the ends
/// repeat beyond the clip.
pub(crate) fn smooth<const N: usize>(values: &[[f64; N]], sigma: f64) -> Vec<[f64; N]> {
    if sigma < 0.5 {
        return values.to_vec();
    }
    let reach = (3.0 * sigma).ceil() as i64;
    let weights: Vec<f64> = (-reach..=reach)
        .map(|offset| (-0.5 * (offset as f64 / sigma).powi(2)).exp())
        .collect();
    let total: f64 = weights.iter().sum();
    let last = values.len() as i64 - 1;
    (0..=last)
        .map(|index| {
            let mut sum = [0.0; N];
            for (offset, weight) in (-reach..=reach).zip(&weights) {
                let neighbor = values[(index + offset).clamp(0, last) as usize];
                for (sum, value) in sum.iter_mut().zip(neighbor) {
                    *sum += weight * value;
                }
            }
            sum.map(|sum| sum / total)
        })
        .collect()
}

fn mean(values: &[[f64; 4]]) -> [f64; 4] {
    let count = values.len().max(1) as f64;
    values
        .iter()
        .fold([0.0; 4], |sum, value| {
            [0, 1, 2, 3].map(|axis| sum[axis] + value[axis])
        })
        .map(|sum| sum / count)
}

/// The least enlargement of the output, up to `MAX_ZOOM`, after which `correction`
/// finds every corner of the frame inside the source picture.
fn covering_zoom(correction: &Similarity, [width, height]: [f64; 2]) -> f64 {
    let corners = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]]
        .map(|[x, y]| [x * width / 2.0, y * height / 2.0]);
    let covers = |zoom: f64| {
        corners.iter().all(|&[x, y]| {
            let [x, y] = correction.apply([x / zoom, y / zoom]);
            // A hair of slack keeps rounding from enlarging a still picture.
            x.abs() <= width / 2.0 + 1e-6 && y.abs() <= height / 2.0 + 1e-6
        })
    };
    if covers(1.0) {
        return 1.0;
    }
    if !covers(MAX_ZOOM) {
        return MAX_ZOOM;
    }
    // Shrinking the frame toward the correction's center only moves corners inward
    // once they're inside, so the zooms that cover form one range.
    let (mut low, mut high) = (1.0, MAX_ZOOM);
    for _ in 0..40 {
        let middle = (low + high) / 2.0;
        if covers(middle) {
            high = middle;
        } else {
            low = middle;
        }
    }
    high
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f64; 2], b: [f64; 2]) -> bool {
        (a[0] - b[0]).abs() < 1e-5 && (a[1] - b[1]).abs() < 1e-5
    }

    #[test]
    fn a_similarity_composes_and_inverts() {
        let turn = Similarity {
            shift: [0.1, -0.05],
            angle: 0.3,
            scale: 1.2,
        };
        let nudge = Similarity {
            shift: [0.02, 0.04],
            angle: -0.1,
            scale: 0.9,
        };
        let point = [0.2, 0.15];
        assert!(close(
            turn.then(&nudge).apply(point),
            nudge.apply(turn.apply(point))
        ));
        assert!(close(turn.inverse().apply(turn.apply(point)), point));
    }

    fn shaky_steps(shake: impl Fn(usize) -> [f64; 2]) -> Vec<(Ticks, Similarity)> {
        let mut previous = [0.0, 0.0];
        (0..100)
            .map(|frame| {
                let at = shake(frame);
                let step = Similarity {
                    shift: [at[0] - previous[0], at[1] - previous[1]],
                    ..Similarity::IDENTITY
                };
                previous = at;
                (Ticks(frame as i64 * 3600), step)
            })
            .collect()
    }

    #[test]
    fn smoothing_cancels_shake_but_keeps_a_steady_pan() {
        let shake = |frame: usize| {
            let jitter = if frame.is_multiple_of(2) { 0.01 } else { -0.01 };
            [frame as f64 * 0.001 + jitter, 0.0]
        };
        let mut stabilizer = Stabilizer {
            framing: Framing::StabilizeOnly,
            ..Stabilizer::default()
        };
        stabilizer.analyzed([1920, 1080], &shaky_steps(shake), Timebase::HZ_90K);
        // Away from the ends the correction undoes just the jitter: the stabilized
        // picture's center shows the source where the pan alone would have put it.
        for frame in [40, 41, 50] {
            let time = Ticks(frame as i64 * 3600);
            let correction = stabilizer.correction_at(time).unwrap();
            let jitter = shake(frame)[0] - frame as f64 * 0.001;
            assert!((correction.apply([0.0, 0.0])[0] - jitter).abs() < 1e-3);
        }
    }

    #[test]
    fn auto_scale_enlarges_just_enough_to_hide_the_edges() {
        let shake = |frame: usize| [if frame == 50 { 0.05 } else { 0.0 }, 0.0];
        let mut stabilizer = Stabilizer {
            method: StabilizeMethod::NoMotion,
            ..Stabilizer::default()
        };
        stabilizer.analyzed([1000, 500], &shaky_steps(shake), Timebase::HZ_90K);
        // The worst frame is 0.05 off a picture 1 wide, so its edge needs 0.5 / 0.45.
        let zoom = f64::from(stabilizer.auto_scale) / 1000.0;
        assert!((zoom - 0.5 / 0.45).abs() < 2e-3, "{zoom}");
        let corner = stabilizer
            .correction_at(Ticks(50 * 3600))
            .unwrap()
            .apply([0.5, 0.25]);
        assert!(corner[0] <= 0.5 + 1e-5 && corner[1] <= 0.25 + 1e-5);
    }
}
