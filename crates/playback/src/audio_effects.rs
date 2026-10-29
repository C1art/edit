use std::mem::{discriminant, Discriminant};

use audio::{
    Biquad, Coefficients, Compressor, CompressorSettings, DeEsser, DeHum, DeNoise, Delay, Gate,
    GraphicEqualizer, Limiter, ModulatedDelay, MultibandCompressor, MultibandSettings,
    PitchShifter, Reverb, SampleRate, Sweep,
};
use timeline::{AudioEffect, ClipId, Ticks};

use crate::audio_effects_more::MoreUnit;

/// Corner frequencies of the equalizer's shelves.
const LOW_SHELF_HZ: f32 = 100.0;
const HIGH_SHELF_HZ: f32 = 8_000.0;
/// Where the Bass and Treble effects' shelves turn.
const BASS_HZ: f32 = 200.0;
const TREBLE_HZ: f32 = 4_000.0;
/// The equalizer's bell spans about an octave and a third.
const BELL_Q: f32 = 1.0;
/// The fixed part of each modulated delay, under its sweep.
const CHORUS_BASE_MS: f32 = 15.0;
const FLANGER_BASE_MS: f32 = 1.0;
/// A Butterworth response: flat, then rolling off without a bump.
const BUTTERWORTH_Q: f32 = std::f32::consts::FRAC_1_SQRT_2;

enum Unit {
    Equalizer([Biquad; 3]),
    Graphic(Box<GraphicEqualizer>),
    Multiband(Box<MultibandCompressor>),
    Filter(Biquad),
    Compressor(Compressor),
    Delay(Box<Delay>),
    Limiter(Limiter),
    Gate(Gate),
    Reverb(Box<Reverb>),
    DeEsser(DeEsser),
    Sweep(Box<ModulatedDelay>),
    Pitch(Box<PitchShifter>),
    DeNoise(Box<DeNoise>),
    DeHum(Box<DeHum>),
    More(MoreUnit),
    /// For effects that keep nothing from one sample to the next.
    Stateless,
}

/// The running state of one clip's audio effects, carried from block to block.
pub(crate) struct EffectChain {
    rate: SampleRate,
    /// The clip the effects belong to; none for a track's inserts.
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
                Unit::Graphic(equalizer) => equalizer.reset(),
                Unit::Multiband(compressor) => compressor.reset(),
                Unit::Filter(filter) => filter.reset(),
                Unit::Compressor(compressor) => compressor.reset(),
                Unit::Delay(delay) => delay.reset(),
                Unit::Limiter(limiter) => limiter.reset(),
                Unit::Gate(gate) => gate.reset(),
                Unit::Reverb(reverb) => reverb.reset(),
                Unit::DeEsser(deesser) => deesser.reset(),
                Unit::Sweep(sweep) => sweep.reset(),
                Unit::Pitch(shifter) => shifter.reset(),
                Unit::DeNoise(denoise) => denoise.reset(),
                Unit::DeHum(dehum) => dehum.reset(),
                Unit::More(unit) => unit.reset(),
                Unit::Stateless => {}
            }
        }
    }

    /// Runs `owner`'s `effects`, set as they are at `time` (source time for a clip,
    /// sequence time for a track), over interleaved stereo `samples`.
    pub(crate) fn process(
        &mut self,
        owner: Option<ClipId>,
        effects: &[AudioEffect],
        time: Ticks,
        samples: &mut [f32],
    ) {
        self.follow(owner, effects);
        let rate = self.rate;
        for (effect, unit) in effects.iter().zip(&mut self.units) {
            match (effect, unit) {
                (AudioEffect::Amplify { gain }, Unit::Stateless) => {
                    let gain = db_to_amplitude(gain.value_at(time));
                    samples.iter_mut().for_each(|sample| *sample *= gain);
                }
                (AudioEffect::Balance { balance }, Unit::Stateless) => {
                    let balance = (balance.value_at(time) / 100.0).clamp(-1.0, 1.0);
                    let sides = [1.0 - balance.max(0.0), 1.0 + balance.min(0.0)];
                    for frame in samples.as_chunks_mut::<2>().0 {
                        frame[0] *= sides[0];
                        frame[1] *= sides[1];
                    }
                }
                (AudioEffect::Bass { boost }, Unit::Filter(filter)) => {
                    let db = boost.value_at(time);
                    filter.set(Coefficients::low_shelf(rate, BASS_HZ, db));
                    filter.process(samples);
                }
                (AudioEffect::Treble { boost }, Unit::Filter(filter)) => {
                    let db = boost.value_at(time);
                    filter.set(Coefficients::high_shelf(rate, TREBLE_HZ, db));
                    filter.process(samples);
                }
                (AudioEffect::Notch { frequency, q }, Unit::Filter(filter)) => {
                    let (hz, q) = (frequency.value_at(time), q.value_at(time).max(0.1));
                    filter.set(Coefficients::notch(rate, hz, q));
                    filter.process(samples);
                }
                (
                    AudioEffect::HardLimiter {
                        boost,
                        ceiling,
                        release,
                    },
                    Unit::Limiter(limiter),
                ) => limiter.process(
                    boost.value_at(time),
                    ceiling.value_at(time),
                    release.value_at(time),
                    rate,
                    samples,
                ),
                (
                    AudioEffect::NoiseGate {
                        threshold,
                        attack,
                        release,
                    },
                    Unit::Gate(gate),
                ) => gate.process(
                    threshold.value_at(time),
                    attack.value_at(time),
                    release.value_at(time),
                    rate,
                    samples,
                ),
                (AudioEffect::Reverb { size, damping, mix }, Unit::Reverb(reverb)) => reverb
                    .process(
                        size.value_at(time) / 100.0,
                        damping.value_at(time) / 100.0,
                        mix.value_at(time) / 100.0,
                        samples,
                    ),
                (AudioEffect::ChannelVolume { left, right }, Unit::Stateless) => {
                    let sides = [left, right].map(|side| db_to_amplitude(side.value_at(time)));
                    for frame in samples.as_chunks_mut::<2>().0 {
                        frame[0] *= sides[0];
                        frame[1] *= sides[1];
                    }
                }
                (AudioEffect::GraphicEqualizer { bands }, Unit::Graphic(equalizer)) => {
                    let gains = bands.each_ref().map(|band| band.value_at(time));
                    equalizer.process(gains, rate, samples);
                }
                (
                    AudioEffect::MultibandCompressor {
                        low_crossover,
                        high_crossover,
                        low_threshold,
                        mid_threshold,
                        high_threshold,
                        ratio,
                        attack,
                        release,
                        makeup,
                    },
                    Unit::Multiband(compressor),
                ) => {
                    let band = |threshold: &timeline::Param| CompressorSettings {
                        threshold_db: threshold.value_at(time),
                        ratio: ratio.value_at(time),
                        attack_ms: attack.value_at(time),
                        release_ms: release.value_at(time),
                        makeup_db: makeup.value_at(time),
                    };
                    let settings = MultibandSettings {
                        crossovers_hz: [
                            low_crossover.value_at(time),
                            high_crossover.value_at(time),
                        ],
                        bands: [low_threshold, mid_threshold, high_threshold].map(band),
                    };
                    compressor.process(&settings, rate, samples);
                }
                (AudioEffect::FillLeftWithRight, Unit::Stateless) => {
                    for frame in samples.as_chunks_mut::<2>().0 {
                        frame[0] = frame[1];
                    }
                }
                (AudioEffect::FillRightWithLeft, Unit::Stateless) => {
                    for frame in samples.as_chunks_mut::<2>().0 {
                        frame[1] = frame[0];
                    }
                }
                (AudioEffect::SwapChannels, Unit::Stateless) => {
                    for frame in samples.as_chunks_mut::<2>().0 {
                        frame.swap(0, 1);
                    }
                }
                (AudioEffect::Invert, Unit::Stateless) => {
                    samples.iter_mut().for_each(|sample| *sample = -*sample);
                }
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
                (
                    AudioEffect::DeEsser {
                        threshold,
                        reduction,
                        frequency,
                    },
                    Unit::DeEsser(deesser),
                ) => deesser.process(
                    threshold.value_at(time),
                    reduction.value_at(time),
                    frequency.value_at(time),
                    rate,
                    samples,
                ),
                (
                    AudioEffect::Chorus {
                        rate: speed,
                        depth,
                        mix,
                    },
                    Unit::Sweep(delay),
                ) => {
                    let sweep = Sweep {
                        base_ms: CHORUS_BASE_MS,
                        depth_ms: depth.value_at(time),
                        rate_hz: speed.value_at(time),
                        feedback: 0.0,
                        mix: mix.value_at(time) / 100.0,
                    };
                    delay.process(&sweep, samples);
                }
                (
                    AudioEffect::Flanger {
                        rate: speed,
                        depth,
                        feedback,
                        mix,
                    },
                    Unit::Sweep(delay),
                ) => {
                    let sweep = Sweep {
                        base_ms: FLANGER_BASE_MS,
                        depth_ms: depth.value_at(time),
                        rate_hz: speed.value_at(time),
                        feedback: feedback.value_at(time) / 100.0,
                        mix: mix.value_at(time) / 100.0,
                    };
                    delay.process(&sweep, samples);
                }
                (AudioEffect::PitchShifter { semitones, mix }, Unit::Pitch(shifter)) => shifter
                    .process(
                        semitones.value_at(time),
                        mix.value_at(time) / 100.0,
                        samples,
                    ),
                (AudioEffect::DeNoise { amount }, Unit::DeNoise(denoise)) => {
                    denoise.process(amount.value_at(time) / 100.0, samples);
                }
                (
                    AudioEffect::DeHum {
                        frequency,
                        harmonics,
                        reduction,
                    },
                    Unit::DeHum(dehum),
                ) => dehum.process(
                    frequency.value_at(time),
                    harmonics.value_at(time).round().max(1.0) as usize,
                    reduction.value_at(time),
                    rate,
                    samples,
                ),
                (effect, Unit::More(unit)) => unit.process(effect, time, rate, samples),
                _ => {}
            }
        }
    }

    /// Rebuilds the units when the chain changes hands or effects are added, removed or
    /// reordered; settings alone change without losing the sound in flight.
    fn follow(&mut self, owner: Option<ClipId>, effects: &[AudioEffect]) {
        let same_kinds = self
            .kinds
            .iter()
            .copied()
            .eq(effects.iter().map(discriminant));
        if self.owner == owner && same_kinds {
            return;
        }
        self.owner = owner;
        self.kinds = effects.iter().map(discriminant).collect();
        self.units = effects.iter().map(|effect| self.unit(effect)).collect();
    }

    fn unit(&self, effect: &AudioEffect) -> Unit {
        match effect {
            AudioEffect::Equalizer { .. } => Unit::Equalizer(Default::default()),
            AudioEffect::GraphicEqualizer { .. } => Unit::Graphic(Box::default()),
            AudioEffect::MultibandCompressor { .. } => Unit::Multiband(Box::default()),
            AudioEffect::HighPass { .. }
            | AudioEffect::LowPass { .. }
            | AudioEffect::Notch { .. }
            | AudioEffect::Bass { .. }
            | AudioEffect::Treble { .. } => Unit::Filter(Biquad::default()),
            AudioEffect::Compressor { .. } => Unit::Compressor(Compressor::default()),
            AudioEffect::Delay { .. } => Unit::Delay(Box::new(Delay::new(self.rate))),
            AudioEffect::HardLimiter { .. } => Unit::Limiter(Limiter::default()),
            AudioEffect::NoiseGate { .. } => Unit::Gate(Gate::default()),
            AudioEffect::Reverb { .. } => Unit::Reverb(Box::new(Reverb::new(self.rate))),
            AudioEffect::DeEsser { .. } => Unit::DeEsser(DeEsser::default()),
            AudioEffect::Chorus { .. } | AudioEffect::Flanger { .. } => {
                Unit::Sweep(Box::new(ModulatedDelay::new(self.rate)))
            }
            AudioEffect::PitchShifter { .. } => Unit::Pitch(Box::new(PitchShifter::new(self.rate))),
            AudioEffect::DeNoise { .. } => Unit::DeNoise(Box::new(DeNoise::new(self.rate))),
            AudioEffect::DeHum { .. } => Unit::DeHum(Box::default()),
            AudioEffect::Amplify { .. }
            | AudioEffect::Balance { .. }
            | AudioEffect::ChannelVolume { .. }
            | AudioEffect::FillLeftWithRight
            | AudioEffect::FillRightWithLeft
            | AudioEffect::SwapChannels
            | AudioEffect::Invert => Unit::Stateless,
            more => MoreUnit::for_effect(more, self.rate).map_or(Unit::Stateless, Unit::More),
        }
    }
}

fn db_to_amplitude(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}
