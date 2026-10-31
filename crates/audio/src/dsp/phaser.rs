use std::f32::consts::{PI, TAU};

use crate::SampleRate;

pub const MAX_PHASER_STAGES: usize = 12;
const LOWEST_HZ: f32 = 20.0;

/// How a phaser sweeps and mixes; `feedback` and `mix` are fractions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhaserSettings {
    pub stages: usize,
    /// Where the all-pass filters turn when the sweep is in the middle.
    pub frequency_hz: f32,
    /// How far each way the sweep carries them.
    pub depth_octaves: f32,
    pub rate_hz: f32,
    pub feedback: f32,
    pub mix: f32,
}

/// All-pass filters whose turning frequency sweeps, mixed with the dry sound so that
/// moving notches comb through it. The right side's sweep runs a quarter turn behind
/// the left's.
#[derive(Debug, Clone)]
pub struct Phaser {
    rate: SampleRate,
    /// Each first-order all-pass filter's memory, per side.
    state: [[f32; MAX_PHASER_STAGES]; 2],
    last: [f32; 2],
    phase: f32,
}

impl Phaser {
    pub fn new(rate: SampleRate) -> Self {
        Self {
            rate,
            state: [[0.0; MAX_PHASER_STAGES]; 2],
            last: [0.0; 2],
            phase: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.state = [[0.0; MAX_PHASER_STAGES]; 2];
        self.last = [0.0; 2];
        self.phase = 0.0;
    }

    pub fn process(&mut self, settings: &PhaserSettings, samples: &mut [f32]) {
        let rate = self.rate.0 as f32;
        let stages = settings.stages.clamp(1, MAX_PHASER_STAGES);
        let step = settings.rate_hz.max(0.0) / rate;
        let feedback = settings.feedback.clamp(-0.95, 0.95);
        let mix = settings.mix.clamp(0.0, 1.0);
        for frame in samples.as_chunks_mut::<2>().0 {
            for (side, sample) in frame.iter_mut().enumerate() {
                let swing = (TAU * (self.phase + 0.25 * side as f32)).sin();
                let turn = (settings.frequency_hz * (settings.depth_octaves * swing).exp2())
                    .clamp(LOWEST_HZ, rate * 0.45);
                let tangent = (PI * turn / rate).tan();
                let coefficient = (tangent - 1.0) / (tangent + 1.0);
                let mut wet = *sample + self.last[side] * feedback;
                for memory in &mut self.state[side][..stages] {
                    let output = coefficient * wet + *memory;
                    *memory = super::flush(wet - coefficient * output);
                    wet = output;
                }
                self.last[side] = super::flush(wet);
                *sample += (wet - *sample) * mix;
            }
            self.phase = (self.phase + step).fract();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settled_peak(tone: f32, settings: &PhaserSettings) -> f32 {
        let rate = SampleRate(48_000);
        let mut phaser = Phaser::new(rate);
        let mut samples: Vec<f32> = (0..9_600)
            .flat_map(|frame| [(TAU * tone * frame as f32 / rate.0 as f32).sin(); 2])
            .collect();
        phaser.process(settings, &mut samples);
        samples[4_800..]
            .iter()
            .fold(0.0, |peak, sample| peak.max(sample.abs()))
    }

    #[test]
    fn two_still_stages_notch_out_their_turning_frequency() {
        let settings = PhaserSettings {
            stages: 2,
            frequency_hz: 1_000.0,
            depth_octaves: 0.0,
            rate_hz: 0.0,
            feedback: 0.0,
            mix: 0.5,
        };
        assert!(settled_peak(1_000.0, &settings) < 0.01);
        assert!(settled_peak(100.0, &settings) > 0.97);
    }
}
