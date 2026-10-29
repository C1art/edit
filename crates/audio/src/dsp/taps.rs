use crate::SampleRate;

use super::delay::MAX_DELAY_MS;

/// One echo of a multitap delay: how long after the sound, and at what fraction of
/// its level.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tap {
    pub time_ms: f32,
    pub level: f32,
}

/// A line of the last `MAX_DELAY_MS` of interleaved stereo.
#[derive(Debug, Clone)]
struct Line {
    rate: SampleRate,
    frames: Vec<[f32; 2]>,
    write: usize,
}

impl Line {
    fn new(rate: SampleRate) -> Self {
        let frames = (MAX_DELAY_MS * 0.001 * rate.0 as f32).ceil() as usize + 1;
        Self {
            rate,
            frames: vec![[0.0; 2]; frames],
            write: 0,
        }
    }

    fn reset(&mut self) {
        self.frames.fill([0.0; 2]);
        self.write = 0;
    }

    fn frames_for(&self, time_ms: f32) -> usize {
        let frames = (time_ms.clamp(0.0, MAX_DELAY_MS) * 0.001 * self.rate.0 as f32).round();
        (frames as usize).clamp(1, self.frames.len() - 1)
    }

    fn read(&self, delay: usize) -> [f32; 2] {
        let length = self.frames.len();
        self.frames[(self.write + length - delay) % length]
    }

    fn push(&mut self, frame: [f32; 2]) {
        self.frames[self.write] = frame;
        self.write = (self.write + 1) % self.frames.len();
    }
}

/// Echoes at four times of their own, each at its own level.
#[derive(Debug, Clone)]
pub struct MultitapDelay {
    line: Line,
}

impl MultitapDelay {
    pub fn new(rate: SampleRate) -> Self {
        Self {
            line: Line::new(rate),
        }
    }

    pub fn reset(&mut self) {
        self.line.reset();
    }

    pub fn process(&mut self, taps: &[Tap; 4], samples: &mut [f32]) {
        let taps = taps.map(|tap| (self.line.frames_for(tap.time_ms), tap.level));
        for frame in samples.as_chunks_mut::<2>().0 {
            let dry = *frame;
            for &(delay, level) in &taps {
                let echo = self.line.read(delay);
                frame[0] += echo[0] * level;
                frame[1] += echo[1] * level;
            }
            self.line.push(dry);
        }
    }
}

/// How a tape-like delay repeats; `feedback`, `saturation` and `mix` are fractions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnalogDelaySettings {
    pub time_ms: f32,
    pub feedback: f32,
    pub saturation: f32,
    /// Above this, each repeat loses more of the highs.
    pub tone_hz: f32,
    pub mix: f32,
}

/// Echoes that darken and saturate as they repeat, as tape and bucket-brigade delays'
/// do.
#[derive(Debug, Clone)]
pub struct AnalogDelay {
    line: Line,
    /// The one-pole low-pass in the feedback path, per side.
    tone: [f32; 2],
}

impl AnalogDelay {
    pub fn new(rate: SampleRate) -> Self {
        Self {
            line: Line::new(rate),
            tone: [0.0; 2],
        }
    }

    pub fn reset(&mut self) {
        self.line.reset();
        self.tone = [0.0; 2];
    }

    pub fn process(&mut self, settings: &AnalogDelaySettings, samples: &mut [f32]) {
        let delay = self.line.frames_for(settings.time_ms);
        let rate = self.line.rate.0 as f32;
        let smoothing = 1.0 - (-std::f32::consts::TAU * settings.tone_hz.max(20.0) / rate).exp();
        let drive = 1.0 + settings.saturation.clamp(0.0, 1.0) * 9.0;
        let feedback = settings.feedback.clamp(0.0, 0.95);
        let mix = settings.mix.clamp(0.0, 1.0);
        for frame in samples.as_chunks_mut::<2>().0 {
            let delayed = self.line.read(delay);
            let mut written = [0.0; 2];
            for side in 0..2 {
                self.tone[side] += (delayed[side] - self.tone[side]) * smoothing;
                let echo = (self.tone[side] * drive).tanh() / drive;
                written[side] = frame[side] + echo * feedback;
                frame[side] += echo * mix;
            }
            self.line.push(written);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_tap_echoes_once_at_its_time_and_level() {
        let mut delay = MultitapDelay::new(SampleRate(1_000));
        let taps = [(5.0, 0.5), (12.0, 0.25), (12.0, 0.25), (30.0, 0.1)]
            .map(|(time_ms, level)| Tap { time_ms, level });
        let mut samples = vec![0.0; 2 * 50];
        samples[0] = 1.0;
        delay.process(&taps, &mut samples);
        let heard: Vec<(usize, f32)> = samples
            .iter()
            .step_by(2)
            .copied()
            .enumerate()
            .filter(|&(_, sample)| sample != 0.0)
            .collect();
        assert_eq!(heard, [(0, 1.0), (5, 0.5), (12, 0.5), (30, 0.1)]);
    }
}
