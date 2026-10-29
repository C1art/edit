use audio::{
    distort, widen, AnalogDelay, AnalogDelaySettings, Biquad, Coefficients, MultitapDelay, Phaser,
    PhaserSettings, SampleRate, Tap,
};
use timeline::{AudioEffect, Param, Ticks};

/// How far each way, in octaves, a phaser's full depth sweeps.
const PHASER_OCTAVES: f32 = 2.0;

/// The running state of the effects added after the first set: bandpass, phaser,
/// distortion, stereo expander and the multitap and analog delays.
pub(crate) enum MoreUnit {
    Filter(Biquad),
    Phaser(Box<Phaser>),
    Multitap(Box<MultitapDelay>),
    Analog(Box<AnalogDelay>),
    Stateless,
}

impl MoreUnit {
    /// The state `effect` keeps, when it is one of these.
    pub(crate) fn for_effect(effect: &AudioEffect, rate: SampleRate) -> Option<Self> {
        Some(match effect {
            AudioEffect::BandPass { .. } => Self::Filter(Biquad::default()),
            AudioEffect::Phaser { .. } => Self::Phaser(Box::new(Phaser::new(rate))),
            AudioEffect::MultitapDelay { .. } => Self::Multitap(Box::new(MultitapDelay::new(rate))),
            AudioEffect::AnalogDelay { .. } => Self::Analog(Box::new(AnalogDelay::new(rate))),
            AudioEffect::Distortion { .. } | AudioEffect::StereoExpander { .. } => Self::Stateless,
            _ => return None,
        })
    }

    pub(crate) fn reset(&mut self) {
        match self {
            Self::Filter(filter) => filter.reset(),
            Self::Phaser(phaser) => phaser.reset(),
            Self::Multitap(delay) => delay.reset(),
            Self::Analog(delay) => delay.reset(),
            Self::Stateless => {}
        }
    }

    pub(crate) fn process(
        &mut self,
        effect: &AudioEffect,
        time: Ticks,
        rate: SampleRate,
        samples: &mut [f32],
    ) {
        let value = |param: &Param| param.value_at(time);
        let fraction = |param: &Param| param.value_at(time) / 100.0;
        match (effect, self) {
            (AudioEffect::BandPass { frequency, q }, Self::Filter(filter)) => {
                let q = value(q).max(0.1);
                filter.set(Coefficients::band_pass(rate, value(frequency), q));
                filter.process(samples);
            }
            (
                AudioEffect::Phaser {
                    stages,
                    frequency,
                    depth,
                    rate: speed,
                    feedback,
                    mix,
                },
                Self::Phaser(phaser),
            ) => {
                let settings = PhaserSettings {
                    stages: value(stages).round().max(1.0) as usize,
                    frequency_hz: value(frequency),
                    depth_octaves: fraction(depth) * PHASER_OCTAVES,
                    rate_hz: value(speed),
                    feedback: fraction(feedback),
                    mix: fraction(mix),
                };
                phaser.process(&settings, samples);
            }
            (AudioEffect::Distortion { drive, mix }, Self::Stateless) => {
                distort(value(drive), fraction(mix), samples);
            }
            (AudioEffect::StereoExpander { width }, Self::Stateless) => {
                widen(fraction(width), samples);
            }
            (AudioEffect::MultitapDelay { times, levels }, Self::Multitap(delay)) => {
                let taps = std::array::from_fn(|index| Tap {
                    time_ms: value(&times[index]),
                    level: fraction(&levels[index]),
                });
                delay.process(&taps, samples);
            }
            (
                AudioEffect::AnalogDelay {
                    time: delay_time,
                    feedback,
                    saturation,
                    tone,
                    mix,
                },
                Self::Analog(delay),
            ) => {
                let settings = AnalogDelaySettings {
                    time_ms: value(delay_time),
                    feedback: fraction(feedback),
                    saturation: fraction(saturation),
                    tone_hz: value(tone),
                    mix: fraction(mix),
                };
                delay.process(&settings, samples);
            }
            _ => {}
        }
    }
}
