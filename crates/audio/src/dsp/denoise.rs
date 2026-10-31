use super::stft::{Spectrum, Stft, BINS, HOP, WINDOW};
use crate::SampleRate;

/// The noise is the quietest each frequency has been over the last `SPANS` spans of
/// `SPAN_SECONDS`, so it follows a noise that rises within about a second.
const SPANS: usize = 8;
const SPAN_SECONDS: f32 = 0.125;
/// Smooths each bin's level across windows for its gain, so single loud windows don't
/// mislead.
const LEVEL_SMOOTHING: f32 = 0.5;
/// Smooths each bin's level over about a tenth of a second for learning the noise, whose
/// level from one window to the next swings too far for its quietest to be its typical.
const NOISE_SMOOTHING: f32 = 0.95;
/// Windows the steady level takes to settle, before which its quietest isn't trusted.
const SETTLING: usize = 20;
const PRIMING: usize = WINDOW / HOP + SETTLING;
/// The quietest a steady noise gets lies about this far under its average.
const MINIMUM_BIAS: f32 = 1.5;
/// At full amount, the noise is subtracted this many times over, so the level swings of
/// noise alone are taken out too.
const OVERSUBTRACTION: f32 = 3.0;
/// Smooths each bin's gain across windows, which tames the warbling "musical noise" of
/// gains that jump.
const GAIN_SMOOTHING: f32 = 0.5;
/// The deepest cut, at full amount.
const MAX_REDUCTION_DB: f32 = 30.0;

/// Lowers steady background noise, such as hiss, fans and room tone: each frequency's
/// noise level is learnt from its quietest moments, and whatever comes near it is turned
/// down. Adds a window of delay.
#[derive(Debug, Clone)]
pub struct DeNoise {
    stft: Stft,
    channels: [Channel; 2],
    learning: Learning,
}

/// Where learning the noise has got to.
#[derive(Debug, Clone)]
struct Learning {
    windows_per_span: usize,
    /// Windows to go before the input fills one again and the steady level settles, after
    /// which the noise is learnt.
    priming: usize,
    /// Windows into the current span, and which span it is.
    span_windows: usize,
    span: usize,
}

#[derive(Debug, Clone)]
struct Channel {
    level: Vec<f32>,
    steady: Vec<f32>,
    /// Each bin's quietest steady level in the current span, then in each past span.
    span_quietest: Vec<f32>,
    past_quietest: Vec<[f32; SPANS]>,
    gain: Vec<f32>,
}

impl Channel {
    fn new() -> Self {
        Self {
            level: vec![0.0; BINS],
            steady: vec![0.0; BINS],
            span_quietest: vec![f32::INFINITY; BINS],
            past_quietest: vec![[f32::INFINITY; SPANS]; BINS],
            gain: vec![1.0; BINS],
        }
    }
}

/// Where a window falls in learning the noise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// The input still holds silence from before a reset.
    Paused,
    /// The first full window since one, whose levels start the smoothing afresh.
    Starting,
    /// The steady level is settling.
    Settling,
    Learning,
    /// The last window of a span, whose quietest levels are then filed.
    SpanEnds,
}

impl DeNoise {
    pub fn new(rate: SampleRate) -> Self {
        let windows_per_second = rate.0 as f32 / HOP as f32;
        Self {
            stft: Stft::new(),
            channels: [Channel::new(), Channel::new()],
            learning: Learning {
                windows_per_span: (windows_per_second * SPAN_SECONDS).round().max(1.0) as usize,
                priming: PRIMING,
                span_windows: 0,
                span: 0,
            },
        }
    }

    /// Forgets the sound in flight, as after a seek, but keeps the noise it learnt.
    pub fn reset(&mut self) {
        self.stft.reset();
        for channel in &mut self.channels {
            channel.gain.fill(1.0);
        }
        self.learning.priming = PRIMING;
    }

    /// Reduces the noise in interleaved stereo `samples` by `amount`, from 0 to 1.
    pub fn process(&mut self, amount: f32, samples: &mut [f32]) {
        let amount = amount.clamp(0.0, 1.0);
        let Self {
            stft,
            channels,
            learning,
        } = self;
        stft.process(samples, |spectra| {
            let phase = learning.next_window();
            for (channel, spectrum) in channels.iter_mut().zip(spectra) {
                channel.shape(spectrum, amount, phase, learning.span);
            }
            if phase == Phase::SpanEnds {
                learning.span = (learning.span + 1) % SPANS;
            }
        });
    }
}

impl Learning {
    fn next_window(&mut self) -> Phase {
        if self.priming > 0 {
            self.priming -= 1;
            return match self.priming {
                left if left > SETTLING => Phase::Paused,
                SETTLING => Phase::Starting,
                _ => Phase::Settling,
            };
        }
        self.span_windows += 1;
        if self.span_windows == self.windows_per_span {
            self.span_windows = 0;
            Phase::SpanEnds
        } else {
            Phase::Learning
        }
    }
}

impl Channel {
    fn shape(&mut self, spectrum: &mut Spectrum, amount: f32, phase: Phase, span: usize) {
        let floor = 10.0_f32.powf(-MAX_REDUCTION_DB * amount / 20.0);
        for bin in 0..BINS {
            let power = spectrum.power(bin);
            let smooth = |previous: f32, smoothing: f32| match phase {
                Phase::Paused | Phase::Starting => power,
                Phase::Settling | Phase::Learning | Phase::SpanEnds => {
                    smoothing * previous + (1.0 - smoothing) * power
                }
            };
            let level = smooth(self.level[bin], LEVEL_SMOOTHING);
            self.level[bin] = level;
            if phase != Phase::Paused {
                self.steady[bin] = smooth(self.steady[bin], NOISE_SMOOTHING);
            }
            if matches!(phase, Phase::Learning | Phase::SpanEnds) {
                self.span_quietest[bin] = self.span_quietest[bin].min(self.steady[bin]);
            }
            let past = &mut self.past_quietest[bin];
            let noise = past
                .iter()
                .fold(self.span_quietest[bin], |quietest, &span| {
                    quietest.min(span)
                })
                * MINIMUM_BIAS;
            if phase == Phase::SpanEnds {
                past[span] = self.span_quietest[bin];
                self.span_quietest[bin] = f32::INFINITY;
            }
            // Nothing is cut before any noise has been heard.
            let wanted = if noise.is_finite() && level > 0.0 {
                (1.0 - OVERSUBTRACTION * amount * noise / level).max(floor)
            } else {
                1.0
            };
            let gain = &mut self.gain[bin];
            *gain = GAIN_SMOOTHING * *gain + (1.0 - GAIN_SMOOTHING) * wanted;
            spectrum.scale(bin, *gain);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::TAU;

    use super::*;

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32).sqrt()
    }

    #[test]
    fn hiss_is_lowered_while_a_tone_through_it_keeps_its_level() {
        let rate = 48_000;
        let mut seed = 1_u32;
        let mut hiss = || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (seed >> 8) as f32 / (1 << 24) as f32 * 0.04 - 0.02
        };
        // A second of hiss alone, then a second with a tone over it.
        let tone = |n: usize| {
            if n >= rate {
                0.3 * (TAU * 1_000.0 * n as f32 / rate as f32).sin()
            } else {
                0.0
            }
        };
        let mut samples: Vec<f32> = (0..2 * rate)
            .flat_map(|n| {
                let value = tone(n) + hiss();
                [value, value]
            })
            .collect();
        let hiss_before = rms(&samples[rate..2 * rate]);
        DeNoise::new(SampleRate(rate as u32)).process(1.0, &mut samples);
        let hiss_after = rms(&samples[rate..2 * rate]);
        assert!(
            hiss_after < hiss_before * 0.1,
            "hiss {hiss_before} became {hiss_after}"
        );
        let tone_after = rms(&samples[3 * rate..4 * rate]);
        let tone_level = 0.3 / 2.0_f32.sqrt();
        assert!(
            (tone_after / tone_level - 1.0).abs() < 0.1,
            "tone {tone_level} became {tone_after}"
        );
    }
}
