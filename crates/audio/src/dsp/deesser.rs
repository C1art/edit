use super::{db_to_amplitude, Biquad, Coefficients};
use crate::SampleRate;

const ATTACK_MS: f32 = 1.0;
const RELEASE_MS: f32 = 60.0;
const BUTTERWORTH_Q: f32 = std::f32::consts::FRAC_1_SQRT_2;

fn smoothing(ms: f32, rate: SampleRate) -> f32 {
    (-1.0 / (ms * 0.001 * rate.0 as f32)).exp()
}

/// Turns down the hiss of s and sh sounds: whatever lies above a frequency is lowered
/// while it rises over a threshold, leaving the rest of the voice alone.
#[derive(Debug, Clone, Default)]
pub struct DeEsser {
    /// A steep low-pass; what it leaves out is the band turned down, so the two parts
    /// always add back up to the input.
    split: [Biquad; 2],
    highs: Vec<f32>,
    level: f32,
}

impl DeEsser {
    pub fn reset(&mut self) {
        self.split.iter_mut().for_each(Biquad::reset);
        self.level = 0.0;
    }

    /// Lowers what lies above `frequency` hertz by up to `reduction_db` while it is louder
    /// than `threshold_db`.
    pub fn process(
        &mut self,
        threshold_db: f32,
        reduction_db: f32,
        frequency: f32,
        rate: SampleRate,
        samples: &mut [f32],
    ) {
        self.highs.clear();
        self.highs.extend_from_slice(samples);
        for filter in &mut self.split {
            filter.set(Coefficients::low_pass(rate, frequency, BUTTERWORTH_Q));
            filter.process(&mut self.highs);
        }
        for (high, sample) in self.highs.iter_mut().zip(samples.iter()) {
            *high = sample - *high;
        }
        let (attack, release) = (smoothing(ATTACK_MS, rate), smoothing(RELEASE_MS, rate));
        let threshold = db_to_amplitude(threshold_db);
        let deepest = db_to_amplitude(-reduction_db.max(0.0));
        let frames = samples.as_chunks_mut::<2>().0.iter_mut();
        for (frame, highs) in frames.zip(self.highs.as_chunks::<2>().0) {
            let peak = highs[0].abs().max(highs[1].abs());
            let coefficient = if peak > self.level { attack } else { release };
            self.level = super::flush(peak + coefficient * (self.level - peak));
            let gain = if self.level > threshold {
                (threshold / self.level).max(deepest)
            } else {
                1.0
            };
            for (sample, high) in frame.iter_mut().zip(highs) {
                *sample -= high * (1.0 - gain);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peak_after_settling(frequency: f32) -> f32 {
        let rate = SampleRate(48_000);
        let samples: Vec<f32> = (0..9_600)
            .flat_map(|index| {
                let value =
                    0.5 * (std::f32::consts::TAU * frequency * index as f32 / 48_000.0).sin();
                [value, value]
            })
            .collect();
        let mut processed = samples;
        DeEsser::default().process(-30.0, 12.0, 5_000.0, rate, &mut processed);
        processed[processed.len() / 2..]
            .iter()
            .fold(0.0, |peak, sample| sample.abs().max(peak))
    }

    #[test]
    fn hiss_is_turned_down_and_low_voice_left_alone() {
        assert!(peak_after_settling(9_000.0) < 0.2);
        assert!(peak_after_settling(300.0) > 0.45);
    }
}
