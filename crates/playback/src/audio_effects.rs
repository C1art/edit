use std::mem::{discriminant, Discriminant};

use audio::{Biquad, Coefficients, Compressor, CompressorSettings, Delay, SampleRate};
use timeline::{AudioEffect, ClipId, Ticks};

/// Corner frequencies of the equalizer's shelves.
const LOW_SHELF_HZ: f32 = 100.0;
const HIGH_SHELF_HZ: f32 = 8_000.0;
/// The equalizer's bell spans about an octave and a third.
const BELL_Q: f32 = 1.0;
/// A Butterworth response: flat, then rolling off without a bump.
const BUTTERWORTH_Q: f32 = std::f32::consts::FRAC_1_SQRT_2;

enum Unit {
    Equalizer([Biquad; 3]),
    Filter(Biquad),
    Compressor(Compressor),
    Delay(Box<Delay>),
}

/// The running state of one clip's audio effects, carried from block to block.
pub(crate) struct EffectChain {
    rate: SampleRate,
    owner: Option<ClipId>,
    kinds: Vec<Discriminant<AudioEffect>>,
    units: Vec<Unit>,
}

impl EffectChain {
    pub(crate) fn new(rate: SampleRate) -> Self {
        Self {
            rate,
            owner: None,
            kinds: Vec::new(),
            units: Vec::new(),
        }
    }

    /// Forgets the sound so far, as after a seek.
    pub(crate) fn reset(&mut self) {
        for unit in &mut self.units {
            match unit {
                Unit::Equalizer(bands) => bands.iter_mut().for_each(Biquad::reset),
                Unit::Filter(filter) => filter.reset(),
                Unit::Compressor(compressor) => compressor.reset(),
                Unit::Delay(delay) => delay.reset(),
            }
        }
    }

    /// Runs `clip`'s `effects`, set as they are at source time `time`, over interleaved
    /// stereo `samples`.
    pub(crate) fn process(
        &mut self,
        clip: ClipId,
        effects: &[AudioEffect],
        time: Ticks,
        samples: &mut [f32],
    ) {
        self.follow(clip, effects);
        let rate = self.rate;
        for (effect, unit) in effects.iter().zip(&mut self.units) {
            match (effect, unit) {
                (
                    AudioEffect::Equalizer {
                        low,
                        mid,
                        mid_frequency,
                        high,
                    },
                    Unit::Equalizer(bands),
                ) => {
                    let mid_hz = mid_frequency.value_at(time);
                    bands[0].set(Coefficients::low_shelf(
                        rate,
                        LOW_SHELF_HZ,
                        low.value_at(time),
                    ));
                    bands[1].set(Coefficients::peaking(
                        rate,
                        mid_hz,
                        BELL_Q,
                        mid.value_at(time),
                    ));
                    bands[2].set(Coefficients::high_shelf(
                        rate,
                        HIGH_SHELF_HZ,
                        high.value_at(time),
                    ));
                    bands.iter_mut().for_each(|band| band.process(samples));
                }
                (AudioEffect::HighPass { frequency }, Unit::Filter(filter)) => {
                    let hz = frequency.value_at(time);
                    filter.set(Coefficients::high_pass(rate, hz, BUTTERWORTH_Q));
                    filter.process(samples);
                }
                (AudioEffect::LowPass { frequency }, Unit::Filter(filter)) => {
                    let hz = frequency.value_at(time);
                    filter.set(Coefficients::low_pass(rate, hz, BUTTERWORTH_Q));
                    filter.process(samples);
                }
                (
                    AudioEffect::Compressor {
                        threshold,
                        ratio,
                        attack,
                        release,
                        makeup,
                    },
                    Unit::Compressor(compressor),
                ) => {
                    let settings = CompressorSettings {
                        threshold_db: threshold.value_at(time),
                        ratio: ratio.value_at(time),
                        attack_ms: attack.value_at(time),
                        release_ms: release.value_at(time),
                        makeup_db: makeup.value_at(time),
                    };
                    compressor.process(&settings, rate, samples);
                }
                (
                    AudioEffect::Delay {
                        time: delay_time,
                        feedback,
                        mix,
                    },
                    Unit::Delay(delay),
                ) => delay.process(
                    delay_time.value_at(time),
                    feedback.value_at(time) / 100.0,
                    mix.value_at(time) / 100.0,
                    samples,
                ),
                _ => {}
            }
        }
    }

    /// Rebuilds the units when the chain changes hands or effects are added, removed or
    /// reordered; settings alone change without losing the sound in flight.
    fn follow(&mut self, clip: ClipId, effects: &[AudioEffect]) {
        let same_kinds = self
            .kinds
            .iter()
            .copied()
            .eq(effects.iter().map(discriminant));
        if self.owner == Some(clip) && same_kinds {
            return;
        }
        self.owner = Some(clip);
        self.kinds = effects.iter().map(discriminant).collect();
        self.units = effects.iter().map(|effect| self.unit(effect)).collect();
    }

    fn unit(&self, effect: &AudioEffect) -> Unit {
        match effect {
            AudioEffect::Equalizer { .. } => Unit::Equalizer(Default::default()),
            AudioEffect::HighPass { .. } | AudioEffect::LowPass { .. } => {
                Unit::Filter(Biquad::default())
            }
            AudioEffect::Compressor { .. } => Unit::Compressor(Compressor::default()),
            AudioEffect::Delay { .. } => Unit::Delay(Box::new(Delay::new(self.rate))),
        }
    }
}
