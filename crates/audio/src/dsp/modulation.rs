use std::f32::consts::TAU;

use crate::SampleRate;

/// The longest swept delay, which sets the memory a modulated delay keeps.
pub const MAX_SWEEP_MS: f32 = 60.0;

/// How a modulated delay sweeps and mixes; `feedback` and `mix` are fractions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sweep {
    pub base_ms: f32,
    pub depth_ms: f32,
    pub rate_hz: f32,
    pub feedback: f32,
    pub mix: f32,
}

/// A delay whose length sweeps back and forth, for chorus and flanging. The right
/// side's sweep runs a quarter turn behind the left's to widen the sound.
#[derive(Debug, Clone)]
pub struct ModulatedDelay {
    rate: SampleRate,
    line: Vec<[f32; 2]>,
    write: usize,
    phase: f32,
}

impl ModulatedDelay {
    pub fn new(rate: SampleRate) -> Self {
        let frames = (MAX_SWEEP_MS * 0.001 * rate.0 as f32).ceil() as usize + 2;
        Self {
            rate,
            line: vec![[0.0; 2]; frames],
            write: 0,
            phase: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.line.fill([0.0; 2]);
        self.write = 0;
        self.phase = 0.0;
    }

    pub fn process(&mut self, sweep: &Sweep, samples: &mut [f32]) {
        let length = self.line.len();
        let per_ms = self.rate.0 as f32 * 0.001;
        let step = sweep.rate_hz.max(0.0) / self.rate.0 as f32;
        let feedback = sweep.feedback.clamp(-0.95, 0.95);
        let mix = sweep.mix.clamp(0.0, 1.0);
        let longest = (length - 2) as f32;
        for frame in samples.as_chunks_mut::<2>().0 {
            for (side, sample) in frame.iter_mut().enumerate() {
                let swing = 0.5 + 0.5 * (TAU * (self.phase + 0.25 * side as f32)).sin();
                let delay = ((sweep.base_ms + sweep.depth_ms * swing) * per_ms).clamp(1.0, longest);
                let delayed = read(&self.line, self.write, delay, side);
                self.line[self.write][side] = super::flush(*sample + delayed * feedback);
                *sample += (delayed - *sample) * mix;
            }
            self.write = (self.write + 1) % length;
            self.phase = (self.phase + step).fract();
        }
    }
}

/// The sample `delay` frames before `write` on one side, between whole frames by
/// straight-line interpolation.
pub(super) fn read(line: &[[f32; 2]], write: usize, delay: f32, side: usize) -> f32 {
    let length = line.len();
    let back = delay.floor();
    let fraction = delay - back;
    let newer = (write + length - back as usize) % length;
    let older = (newer + length - 1) % length;
    line[newer][side] + (line[older][side] - line[newer][side]) * fraction
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_still_sweep_echoes_once_at_its_delay_between_frames() {
        let mut delay = ModulatedDelay::new(SampleRate(1_000));
        let sweep = Sweep {
            base_ms: 10.5,
            depth_ms: 0.0,
            rate_hz: 0.0,
            feedback: 0.0,
            mix: 1.0,
        };
        let mut samples = vec![0.0; 2 * 40];
        samples[0] = 1.0;
        delay.process(&sweep, &mut samples);
        let left: Vec<f32> = samples.iter().step_by(2).copied().collect();
        assert_eq!(&left[9..13], &[0.0, 0.5, 0.5, 0.0]);
    }
}
