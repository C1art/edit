use crate::SampleRate;

/// The longest delay, which sets the memory a delay keeps.
pub const MAX_DELAY_MS: f32 = 2_000.0;

/// Echoes of interleaved stereo, each fed back into the line.
#[derive(Debug, Clone)]
pub struct Delay {
    rate: SampleRate,
    line: Vec<[f32; 2]>,
    write: usize,
}

impl Delay {
    pub fn new(rate: SampleRate) -> Self {
        let frames = (MAX_DELAY_MS * 0.001 * rate.0 as f32).ceil() as usize + 1;
        Self {
            rate,
            line: vec![[0.0; 2]; frames],
            write: 0,
        }
    }

    pub fn reset(&mut self) {
        self.line.fill([0.0; 2]);
        self.write = 0;
    }

    /// Adds echoes `time_ms` apart, each `feedback` of the one before, at `mix` of the
    /// input's level; `feedback` and `mix` are fractions.
    pub fn process(&mut self, time_ms: f32, feedback: f32, mix: f32, samples: &mut [f32]) {
        let length = self.line.len();
        let delay = ((time_ms.clamp(0.0, MAX_DELAY_MS) * 0.001 * self.rate.0 as f32).round()
            as usize)
            .clamp(1, length - 1);
        let feedback = feedback.clamp(0.0, 0.95);
        for frame in samples.as_chunks_mut::<2>().0 {
            let delayed = self.line[(self.write + length - delay) % length];
            for side in 0..2 {
                self.line[self.write][side] = super::flush(frame[side] + delayed[side] * feedback);
                frame[side] += delayed[side] * mix;
            }
            self.write = (self.write + 1) % length;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echoes_repeat_at_the_delay_and_decay_by_the_feedback() {
        let mut delay = Delay::new(SampleRate(1_000));
        let mut samples = vec![0.0; 2 * 100];
        samples[0] = 1.0;
        delay.process(10.0, 0.5, 0.8, &mut samples);
        let left: Vec<(usize, f32)> = samples
            .iter()
            .step_by(2)
            .copied()
            .enumerate()
            .filter(|&(_, sample)| sample != 0.0)
            .take(3)
            .collect();
        assert_eq!(left, [(0, 1.0), (10, 0.8), (20, 0.4)]);
    }
}
