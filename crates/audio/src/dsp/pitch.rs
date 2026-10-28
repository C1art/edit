use std::f32::consts::PI;

use super::modulation::read;
use crate::SampleRate;

/// The span each read-head sweeps before the other takes over; longer smooths low
/// notes, shorter smears transients less.
const WINDOW_MS: f32 = 40.0;

/// Shifts pitch without changing timing: two read-heads slide through a short delay at
/// the new speed, each faded out as it wraps around while the other plays.
#[derive(Debug, Clone)]
pub struct PitchShifter {
    window: f32,
    line: Vec<[f32; 2]>,
    write: usize,
    phase: f32,
}

impl PitchShifter {
    pub fn new(rate: SampleRate) -> Self {
        let window = WINDOW_MS * 0.001 * rate.0 as f32;
        Self {
            window,
            line: vec![[0.0; 2]; window.ceil() as usize + 3],
            write: 0,
            phase: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.line.fill([0.0; 2]);
        self.write = 0;
        self.phase = 0.0;
    }

    /// Raises the pitch by `semitones` (lowers when negative), mixed in at `mix`.
    pub fn process(&mut self, semitones: f32, mix: f32, samples: &mut [f32]) {
        let ratio = 2.0_f32.powf(semitones.clamp(-24.0, 24.0) / 12.0);
        let step = (1.0 - ratio) / self.window;
        let mix = mix.clamp(0.0, 1.0);
        for frame in samples.as_chunks_mut::<2>().0 {
            self.line[self.write] = *frame;
            for (side, sample) in frame.iter_mut().enumerate() {
                let shifted: f32 = [self.phase, (self.phase + 0.5).fract()]
                    .into_iter()
                    .map(|head| {
                        let fade = (PI * head).sin().powi(2);
                        fade * read(&self.line, self.write, 1.0 + head * self.window, side)
                    })
                    .sum();
                *sample += (shifted - *sample) * mix;
            }
            self.write = (self.write + 1) % self.line.len();
            self.phase = (self.phase + step).rem_euclid(1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_octave_up_doubles_the_frequency() {
        let rate = 8_000;
        let mut samples: Vec<f32> = (0..rate)
            .flat_map(|index| {
                let value = (std::f32::consts::TAU * 200.0 * index as f32 / rate as f32).sin();
                [value, value]
            })
            .collect();
        PitchShifter::new(SampleRate(rate)).process(12.0, 1.0, &mut samples);
        let left: Vec<f32> = samples.iter().step_by(2).copied().collect();
        let rises = left[800..]
            .windows(2)
            .filter(|pair| pair[0] < 0.0 && pair[1] >= 0.0)
            .count();
        // 0.9 s at 400 Hz.
        assert!((340..=380).contains(&rises), "{rises} rising crossings");
    }
}
