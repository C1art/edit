use super::{Biquad, Coefficients};
use crate::SampleRate;

/// The most harmonics it removes, the fundamental counting as the first.
pub const MAX_HARMONICS: usize = 8;
/// Narrow enough to leave a voice's own low notes alone.
const NOTCH_Q: f32 = 30.0;

/// Removes mains hum, the drone at 50 or 60 hertz that poor grounding adds, and its
/// harmonics.
#[derive(Debug, Clone, Default)]
pub struct DeHum {
    filters: [Biquad; MAX_HARMONICS],
}

impl DeHum {
    pub fn reset(&mut self) {
        self.filters.iter_mut().for_each(Biquad::reset);
    }

    /// Cuts `frequency` hertz and its next `harmonics - 1` multiples by `reduction_db`.
    pub fn process(
        &mut self,
        frequency: f32,
        harmonics: usize,
        reduction_db: f32,
        rate: SampleRate,
        samples: &mut [f32],
    ) {
        let nyquist = rate.0 as f32 / 2.0;
        let used = harmonics.clamp(1, MAX_HARMONICS);
        for (multiple, filter) in (1..).zip(&mut self.filters[..used]) {
            let hz = frequency * multiple as f32;
            if hz >= nyquist * 0.9 {
                break;
            }
            filter.set(Coefficients::peaking(
                rate,
                hz,
                NOTCH_Q,
                -reduction_db.max(0.0),
            ));
            filter.process(samples);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::TAU;

    use super::*;

    /// The level of `hz` in a mono signal, by correlating with a sine and cosine.
    fn level_at(samples: &[f32], hz: f32, rate: f32) -> f32 {
        let (mut re, mut im) = (0.0, 0.0);
        for (n, sample) in samples.iter().enumerate() {
            let angle = TAU * hz * n as f32 / rate;
            re += sample * angle.cos();
            im += sample * angle.sin();
        }
        2.0 * re.hypot(im) / samples.len() as f32
    }

    #[test]
    fn hum_and_its_harmonics_go_while_the_voice_band_stays() {
        let rate = 48_000.0;
        let signal = |n: usize| {
            let t = n as f32 / rate;
            0.2 * (TAU * 60.0 * t).sin()
                + 0.1 * (TAU * 180.0 * t).sin()
                + 0.2 * (TAU * 1_000.0 * t).sin()
        };
        let mut samples: Vec<f32> = (0..48_000).flat_map(|n| [signal(n); 2]).collect();
        DeHum::default().process(60.0, 4, 40.0, SampleRate(48_000), &mut samples);
        let settled: Vec<f32> = samples[24_000..].iter().step_by(2).copied().collect();
        assert!(level_at(&settled, 60.0, rate) < 0.01);
        assert!(level_at(&settled, 180.0, rate) < 0.005);
        assert!((level_at(&settled, 1_000.0, rate) - 0.2).abs() < 0.01);
    }
}
