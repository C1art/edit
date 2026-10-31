use audio::{
    distort, widen, AnalogDelay, AnalogDelaySettings, Biquad, ClickRemover, Coefficients,
    Compressor, CompressorSettings, DeReverb, FilterBand, FilterDesign, GraphicEqualizer20,
    GraphicEqualizer30, LevelerSettings, MultitapDelay, Phaser, PhaserSettings, SampleRate,
    ScientificFilter, ScientificSettings, SpeechLeveler, Tap,
};
use timeline::{AudioEffect, FilterMode, FilterType, Param, Ticks};

use crate::audio_convolution::{ConvolutionReverb, ReverbLevels, SpaceSettings};
use crate::audio_dynamics::{DynamicsChain, VocalChain};
use crate::audio_effects::db_to_amplitude;
use crate::audio_guitar::{GuitarChain, GuitarSettings};
use crate::audio_mastering::{MasteringChain, MasteringSettings};

/// How far each way, in octaves, a phaser's full depth sweeps.
const PHASER_OCTAVES: f32 = 2.0;
/// How hard the Tube-modeled Compressor drives its saturation, which its warmth mixes in.
const TUBE_DRIVE_DB: f32 = 12.0;
/// How many times further than the sound's usual roughness a click stands out at the
/// Automatic Click Remover's lowest threshold, and how much more each percent asks.
const CLICK_SENSITIVITY_FLOOR: f32 = 3.0;
const CLICK_SENSITIVITY_PER_PERCENT: f32 = 0.25;

/// The running state of the effects added after the first set: bandpass, phaser,
/// distortion, stereo expander, the multitap and analog delays, Dynamics, Automatic Click
/// Remover, Vocal Enhancer, the Tube-modeled Compressor, DeReverb, Speech Volume Leveler
/// Simple Parametric EQ, the 20- and 30-band Graphic Equalizers, Mastering, the
/// Scientific Filter, the Guitar Suite and the Convolution Reverb.
pub(crate) enum MoreUnit {
    Filter(Biquad),
    Phaser(Box<Phaser>),
    Multitap(Box<MultitapDelay>),
    Analog(Box<AnalogDelay>),
    Dynamics(Box<DynamicsChain>),
    Clicks(ClickRemover),
    Vocal(Box<VocalChain>),
    Compressor(Compressor),
    DeReverb(Box<DeReverb>),
    Leveler(SpeechLeveler),
    Graphic20(Box<GraphicEqualizer20>),
    Graphic30(Box<GraphicEqualizer30>),
    Mastering(Box<MasteringChain>),
    Scientific(Box<ScientificFilter>),
    Guitar(Box<GuitarChain>),
    Convolution(Box<ConvolutionReverb>),
    Stateless,
}

impl MoreUnit {
    /// The state `effect` keeps, when it is one of these.
    pub(crate) fn for_effect(effect: &AudioEffect, rate: SampleRate) -> Option<Self> {
        Some(match effect {
            AudioEffect::BandPass { .. } | AudioEffect::SimpleParametricEq { .. } => {
                Self::Filter(Biquad::default())
            }
            AudioEffect::DeReverb { .. } => Self::DeReverb(Box::new(DeReverb::new(rate))),
            AudioEffect::SpeechVolumeLeveler { .. } => Self::Leveler(SpeechLeveler::default()),
            AudioEffect::GraphicEqualizer20 { .. } => Self::Graphic20(Box::default()),
            AudioEffect::GraphicEqualizer30 { .. } => Self::Graphic30(Box::default()),
            AudioEffect::Mastering { .. } => Self::Mastering(Box::new(MasteringChain::new(rate))),
            AudioEffect::Phaser { .. } => Self::Phaser(Box::new(Phaser::new(rate))),
            AudioEffect::MultitapDelay { .. } => Self::Multitap(Box::new(MultitapDelay::new(rate))),
            AudioEffect::AnalogDelay { .. } => Self::Analog(Box::new(AnalogDelay::new(rate))),
            AudioEffect::Dynamics(_) => Self::Dynamics(Box::default()),
            AudioEffect::ScientificFilter { .. } => Self::Scientific(Box::default()),
            AudioEffect::GuitarSuite { .. } => Self::Guitar(Box::default()),
            AudioEffect::ConvolutionReverb { .. } => {
                let settings = space(effect)?;
                Self::Convolution(Box::new(ConvolutionReverb::new(settings, rate)))
            }
            AudioEffect::AutomaticClickRemover { .. } => Self::Clicks(ClickRemover::default()),
            AudioEffect::VocalEnhancer { .. } => Self::Vocal(Box::default()),
            AudioEffect::TubeCompressor { .. } => Self::Compressor(Compressor::default()),
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
            Self::Dynamics(chain) => chain.reset(),
            Self::Clicks(remover) => remover.reset(),
            Self::Vocal(chain) => chain.reset(),
            Self::Compressor(compressor) => compressor.reset(),
            Self::DeReverb(dereverb) => dereverb.reset(),
            Self::Leveler(leveler) => leveler.reset(),
            Self::Graphic20(equalizer) => equalizer.reset(),
            Self::Graphic30(equalizer) => equalizer.reset(),
            Self::Mastering(chain) => chain.reset(),
            Self::Scientific(filter) => filter.reset(),
            Self::Guitar(chain) => chain.reset(),
            Self::Convolution(reverb) => reverb.reset(),
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
            (AudioEffect::Dynamics(dynamics), Self::Dynamics(chain)) => {
                chain.process(dynamics, time, rate, samples);
            }
            (AudioEffect::AutomaticClickRemover { threshold }, Self::Clicks(remover)) => {
                let sensitivity =
                    CLICK_SENSITIVITY_FLOOR + value(threshold) * CLICK_SENSITIVITY_PER_PERCENT;
                remover.process(sensitivity, samples);
            }
            (AudioEffect::VocalEnhancer { mode }, Self::Vocal(chain)) => {
                chain.process(*mode, rate, samples);
            }
            (
                AudioEffect::TubeCompressor {
                    threshold,
                    ratio,
                    attack,
                    release,
                    makeup,
                    warmth,
                },
                Self::Compressor(compressor),
            ) => {
                let settings = CompressorSettings {
                    threshold_db: value(threshold),
                    ratio: value(ratio),
                    attack_ms: value(attack),
                    release_ms: value(release),
                    makeup_db: value(makeup),
                };
                compressor.process(&settings, rate, samples);
                distort(TUBE_DRIVE_DB, fraction(warmth), samples);
            }
            (AudioEffect::DeReverb { amount, gain }, Self::DeReverb(dereverb)) => {
                dereverb.process(fraction(amount), value(gain), samples);
            }
            (
                AudioEffect::SpeechVolumeLeveler {
                    target,
                    amount,
                    noise_threshold,
                },
                Self::Leveler(leveler),
            ) => {
                let settings = LevelerSettings {
                    target_db: value(target),
                    amount: fraction(amount),
                    noise_threshold_db: value(noise_threshold),
                };
                leveler.process(&settings, rate, samples);
            }
            (AudioEffect::GraphicEqualizer20 { bands }, Self::Graphic20(equalizer)) => {
                equalizer.process(bands.each_ref().map(value), rate, samples);
            }
            (AudioEffect::GraphicEqualizer30 { bands }, Self::Graphic30(equalizer)) => {
                equalizer.process(bands.each_ref().map(value), rate, samples);
            }
            (
                AudioEffect::Mastering {
                    low,
                    mid,
                    high,
                    reverb,
                    exciter,
                    width,
                    maximizer,
                    output,
                },
                Self::Mastering(chain),
            ) => {
                let settings = MasteringSettings {
                    low_db: value(low),
                    mid_db: value(mid),
                    high_db: value(high),
                    reverb: fraction(reverb),
                    exciter: fraction(exciter),
                    width: fraction(width),
                    maximizer: fraction(maximizer),
                    output_db: value(output),
                };
                chain.process(&settings, rate, samples);
            }
            (AudioEffect::SimpleParametricEq { frequency, gain, q }, Self::Filter(filter)) => {
                let q = value(q).max(0.1);
                filter.set(Coefficients::peaking(
                    rate,
                    value(frequency),
                    q,
                    value(gain),
                ));
                filter.process(samples);
            }
            (
                AudioEffect::ScientificFilter {
                    filter_type,
                    mode,
                    order,
                    cutoff,
                    high_cutoff,
                    ripple,
                    gain,
                },
                Self::Scientific(filter),
            ) => {
                let settings = ScientificSettings {
                    design: design(*filter_type),
                    band: band(*mode),
                    order: *order as usize,
                    cutoff: value(cutoff),
                    high_cutoff: value(high_cutoff),
                    ripple_db: value(ripple),
                    gain_db: value(gain),
                };
                filter.process(&settings, rate, samples);
            }
            (
                AudioEffect::GuitarSuite {
                    compressor,
                    distortion,
                    amp,
                    filter,
                    frequency,
                    resonance,
                    mix,
                },
                Self::Guitar(chain),
            ) => {
                let settings = GuitarSettings {
                    compression: fraction(compressor),
                    distortion: fraction(distortion),
                    amp: *amp,
                    filter: *filter,
                    frequency_hz: value(frequency),
                    resonance: fraction(resonance),
                    mix: fraction(mix),
                };
                chain.process(&settings, rate, samples);
            }
            (
                AudioEffect::ConvolutionReverb {
                    width, gain, mix, ..
                },
                Self::Convolution(reverb),
            ) => {
                let levels = ReverbLevels {
                    width: fraction(width),
                    gain: db_to_amplitude(value(gain)),
                    mix: fraction(mix),
                };
                if let Some(settings) = space(effect) {
                    reverb.process(settings, &levels, rate, samples);
                }
            }
            _ => {}
        }
    }
}

fn space(effect: &AudioEffect) -> Option<SpaceSettings> {
    match *effect {
        AudioEffect::ConvolutionReverb {
            space,
            room_size,
            damping,
            pre_delay,
            ..
        } => Some(SpaceSettings {
            space,
            room_size,
            damping,
            pre_delay,
        }),
        _ => None,
    }
}

fn design(filter_type: FilterType) -> FilterDesign {
    match filter_type {
        FilterType::Bessel => FilterDesign::Bessel,
        FilterType::Butterworth => FilterDesign::Butterworth,
        FilterType::Chebyshev => FilterDesign::Chebyshev,
    }
}

fn band(mode: FilterMode) -> FilterBand {
    match mode {
        FilterMode::LowPass => FilterBand::LowPass,
        FilterMode::HighPass => FilterBand::HighPass,
        FilterMode::BandPass => FilterBand::BandPass,
        FilterMode::BandStop => FilterBand::BandStop,
    }
}
