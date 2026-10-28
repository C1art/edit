use std::f64::consts::PI;

use crate::SampleRate;

/// Lowest frequency a filter is set to, below which coefficients lose precision.
const LOWEST_HZ: f64 = 10.0;
/// Shelves with a slope of one, the steepest without overshoot.
const SHELF_SLOPE: f64 = 1.0;

/// A second-order filter's coefficients, normalised so `a0` is one, from the Audio EQ
/// Cookbook.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coefficients {
    b: [f32; 3],
    a: [f32; 2],
}

impl Coefficients {
    pub const IDENTITY: Self = Self {
        b: [1.0, 0.0, 0.0],
        a: [0.0, 0.0],
    };

    pub fn low_pass(rate: SampleRate, frequency: f32, q: f32) -> Self {
        let (cos, alpha) = Self::angle(rate, frequency, f64::from(q));
        let b1 = 1.0 - cos;
        Self::normalised(
            [b1 / 2.0, b1, b1 / 2.0],
            [1.0 + alpha, -2.0 * cos, 1.0 - alpha],
        )
    }

    pub fn high_pass(rate: SampleRate, frequency: f32, q: f32) -> Self {
        let (cos, alpha) = Self::angle(rate, frequency, f64::from(q));
        let b1 = 1.0 + cos;
        Self::normalised(
            [b1 / 2.0, -b1, b1 / 2.0],
            [1.0 + alpha, -2.0 * cos, 1.0 - alpha],
        )
    }

    /// A bell boosting or cutting by `gain_db` around `frequency`.
    pub fn peaking(rate: SampleRate, frequency: f32, q: f32, gain_db: f32) -> Self {
        let (cos, alpha) = Self::angle(rate, frequency, f64::from(q));
        let a = Self::shelf_amplitude(gain_db);
        Self::normalised(
            [1.0 + alpha * a, -2.0 * cos, 1.0 - alpha * a],
            [1.0 + alpha / a, -2.0 * cos, 1.0 - alpha / a],
        )
    }

    /// Boosts or cuts by `gain_db` below `frequency`.
    pub fn low_shelf(rate: SampleRate, frequency: f32, gain_db: f32) -> Self {
        let a = Self::shelf_amplitude(gain_db);
        let (cos, alpha) = Self::angle(rate, frequency, Self::shelf_q(a));
        let root = 2.0 * a.sqrt() * alpha;
        Self::normalised(
            [
                a * ((a + 1.0) - (a - 1.0) * cos + root),
                2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
                a * ((a + 1.0) - (a - 1.0) * cos - root),
            ],
            [
                (a + 1.0) + (a - 1.0) * cos + root,
                -2.0 * ((a - 1.0) + (a + 1.0) * cos),
                (a + 1.0) + (a - 1.0) * cos - root,
            ],
        )
    }

    /// Boosts or cuts by `gain_db` above `frequency`.
    pub fn high_shelf(rate: SampleRate, frequency: f32, gain_db: f32) -> Self {
        let a = Self::shelf_amplitude(gain_db);
        let (cos, alpha) = Self::angle(rate, frequency, Self::shelf_q(a));
        let root = 2.0 * a.sqrt() * alpha;
        Self::normalised(
            [
                a * ((a + 1.0) + (a - 1.0) * cos + root),
                -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
                a * ((a + 1.0) + (a - 1.0) * cos - root),
            ],
            [
                (a + 1.0) - (a - 1.0) * cos + root,
                2.0 * ((a - 1.0) - (a + 1.0) * cos),
                (a + 1.0) - (a - 1.0) * cos - root,
            ],
        )
    }

    /// How much the filter scales a sine at `frequency`.
    pub fn magnitude(&self, rate: SampleRate, frequency: f32) -> f32 {
        let w = 2.0 * PI * f64::from(frequency) / f64::from(rate.0);
        let [b0, b1, b2] = self.b.map(f64::from);
        let [a1, a2] = self.a.map(f64::from);
        let (cos1, sin1, cos2, sin2) = (w.cos(), w.sin(), (2.0 * w).cos(), (2.0 * w).sin());
        let numerator = (b0 + b1 * cos1 + b2 * cos2).hypot(b1 * sin1 + b2 * sin2);
        let denominator = (1.0 + a1 * cos1 + a2 * cos2).hypot(a1 * sin1 + a2 * sin2);
        (numerator / denominator) as f32
    }

    /// The cosine of the centre frequency's angle and the bandwidth term alpha.
    fn angle(rate: SampleRate, frequency: f32, q: f64) -> (f64, f64) {
        let nyquist = f64::from(rate.0) / 2.0;
        let frequency = f64::from(frequency).clamp(LOWEST_HZ, nyquist * 0.98);
        let w = 2.0 * PI * frequency / f64::from(rate.0);
        (w.cos(), w.sin() / (2.0 * q.max(0.01)))
    }

    fn shelf_amplitude(gain_db: f32) -> f64 {
        10.0_f64.powf(f64::from(gain_db) / 40.0)
    }

    fn shelf_q(a: f64) -> f64 {
        1.0 / ((a + 1.0 / a) * (1.0 / SHELF_SLOPE - 1.0) + 2.0).sqrt()
    }

    pub(super) fn normalised(b: [f64; 3], a: [f64; 3]) -> Self {
        let a0 = a[0];
        Self {
            b: b.map(|b| (b / a0) as f32),
            a: [(a[1] / a0) as f32, (a[2] / a0) as f32],
        }
    }
}

/// A second-order filter over interleaved stereo, keeping each side's memory between
/// blocks.
#[derive(Debug, Clone, Copy)]
pub struct Biquad {
    coefficients: Coefficients,
    /// Transposed direct form II state per side.
    state: [[f32; 2]; 2],
}

impl Default for Biquad {
    fn default() -> Self {
        Self {
            coefficients: Coefficients::IDENTITY,
            state: [[0.0; 2]; 2],
        }
    }
}

impl Biquad {
    pub fn set(&mut self, coefficients: Coefficients) {
        self.coefficients = coefficients;
    }

    pub fn reset(&mut self) {
        self.state = [[0.0; 2]; 2];
    }

    pub fn process(&mut self, samples: &mut [f32]) {
        let Coefficients {
            b: [b0, b1, b2],
            a: [a1, a2],
        } = self.coefficients;
        for frame in samples.as_chunks_mut::<2>().0 {
            for (sample, [z1, z2]) in frame.iter_mut().zip(&mut self.state) {
                let input = *sample;
                let output = b0 * input + *z1;
                *z1 = b1 * input - a1 * output + *z2;
                *z2 = b2 * input - a2 * output;
                *sample = output;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: SampleRate = SampleRate(48_000);

    fn db(coefficients: Coefficients, frequency: f32) -> f32 {
        20.0 * coefficients.magnitude(RATE, frequency).log10()
    }

    #[test]
    fn filters_shape_the_bands_they_are_set_for() {
        let low = Coefficients::low_shelf(RATE, 100.0, 6.0);
        assert!((db(low, 20.0) - 6.0).abs() < 0.3);
        assert!(db(low, 5_000.0).abs() < 0.1);
        let high = Coefficients::high_shelf(RATE, 8_000.0, -9.0);
        assert!((db(high, 20_000.0) + 9.0).abs() < 0.5);
        assert!(db(high, 200.0).abs() < 0.1);
        let bell = Coefficients::peaking(RATE, 1_000.0, 1.0, 4.0);
        assert!((db(bell, 1_000.0) - 4.0).abs() < 0.01);
        let cut = Coefficients::high_pass(RATE, 100.0, std::f32::consts::FRAC_1_SQRT_2);
        assert!((db(cut, 100.0) + 3.01).abs() < 0.05);
        assert!(db(cut, 10.0) < -39.0);
    }

    #[test]
    fn processing_matches_the_computed_response() {
        let mut filter = Biquad::default();
        let coefficients = Coefficients::low_pass(RATE, 1_000.0, 0.707);
        filter.set(coefficients);
        let tone = 4_000.0;
        let mut samples: Vec<f32> = (0..9_600)
            .flat_map(|frame| {
                let phase = 2.0 * std::f32::consts::PI * tone * frame as f32 / RATE.0 as f32;
                [phase.sin(); 2]
            })
            .collect();
        filter.process(&mut samples);
        let settled = &samples[4_800..];
        let peak = settled
            .iter()
            .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
        assert!((peak - coefficients.magnitude(RATE, tone)).abs() < 0.005);
    }
}
