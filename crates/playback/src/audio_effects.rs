use std::mem::{discriminant, Discriminant};

use audio::{
    Biquad, Coefficients, Compressor, CompressorSettings, DeEsser, Delay, Gate, Limiter,
    ModulatedDelay, PitchShifter, Reverb, SampleRate, Sweep,
};
use timeline::{AudioEffect, ClipId, Ticks};

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
    Filter(Biquad),
    Compressor(Compressor),
    Delay(Box<Delay>),
    Limiter(Limiter),
    Gate(Gate),
    Reverb(Box<Reverb>),
    DeEsser(DeEsser),
    Sweep(Box<ModulatedDelay>),
    Pitch(Box<PitchShifter>),
    /// For effects that keep nothing from one sample to the next.
    Stateless,
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
                Unit::Limiter(limiter) => limiter.reset(),
                Unit::Gate(gate) => gate.reset(),
                Unit::Reverb(reverb) => reverb.reset(),
                Unit::DeEsser(deesser) => deesser.reset(),
                Unit::Sweep(sweep) => sweep.reset(),
                Unit::Pitch(shifter) => shifter.reset(),
                Unit::Stateless => {}
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
            AudioEffect::Amplify { .. }
            | AudioEffect::Balance { .. }
            | AudioEffect::FillLeftWithRight
            | AudioEffect::FillRightWithLeft
            | AudioEffect::SwapChannels
            | AudioEffect::Invert => Unit::Stateless,
        }
    }
}

fn db_to_amplitude(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}
