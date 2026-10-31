use std::f64::consts::PI;

/// Zero crossings of the low-pass kernel on each side of its centre.
const LOBES: f64 = 4.0;
/// Past this speed the filter stops narrowing, which bounds its taps; what aliases
/// beyond it is buried in sound that fast anyway.
const SPEED_LIMIT: f64 = 16.0;

/// Source frames on each side of a position that `stereo_at` reads at `speed`.
pub(crate) fn reach(speed: f64) -> i64 {
    (LOBES * speed.clamp(1.0, SPEED_LIMIT)).ceil() as i64 + 1
}

/// The source `fraction` past frame `index`, low-passed to the band that still fits
/// below the output's Nyquist frequency once played `speed` times as fast, so its
/// highs don't fold back down as they would with plain interpolation.
pub(crate) fn stereo_at(
    at: impl Fn(i64, usize) -> f32,
    index: i64,
    fraction: f32,
    speed: f64,
) -> [f32; 2] {
    let speed = speed.clamp(1.0, SPEED_LIMIT);
    let centre = f64::from(fraction);
    let half = LOBES * speed;
    let first = (centre - half).floor() as i64 + 1;
    let last = (centre + half).ceil() as i64 - 1;
    // The kernel's sines advance by a fixed angle from tap to tap, so they're rotated
    // on instead of being computed afresh for every tap.
    let start = (first as f64 - centre) / speed;
    let mut wide = Rotation::new(PI * start, PI / speed);
    let mut window = Rotation::new(PI * start / LOBES, PI / (speed * LOBES));
    let (mut sums, mut total) = ([0.0f64; 2], 0.0);
    for (tap, offset) in (first..=last).enumerate() {
        let x = start + tap as f64 / speed;
        let weight = sinc(x, wide.sin) * sinc(x / LOBES, window.sin);
        wide.step();
        window.step();
        total += weight;
        for (channel, sum) in sums.iter_mut().enumerate() {
            *sum += weight * f64::from(at(index + offset, channel));
        }
    }
    if total.abs() < f64::EPSILON {
        return [at(index, 0), at(index, 1)];
    }
    sums.map(|sum| (sum / total) as f32)
}

fn sinc(x: f64, sin_pi_x: f64) -> f64 {
    if x.abs() < 1e-9 {
        1.0
    } else {
        sin_pi_x / (PI * x)
    }
}

struct Rotation {
    sin: f64,
    cos: f64,
    step_sin: f64,
    step_cos: f64,
}

impl Rotation {
    fn new(angle: f64, step: f64) -> Self {
        Self {
            sin: angle.sin(),
            cos: angle.cos(),
            step_sin: step.sin(),
            step_cos: step.cos(),
        }
    }

    fn step(&mut self) {
        let sin = self.sin * self.step_cos + self.cos * self.step_sin;
        self.cos = self.cos * self.step_cos - self.sin * self.step_sin;
        self.sin = sin;
    }
}
