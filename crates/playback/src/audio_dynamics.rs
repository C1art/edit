use audio::{
    Biquad, Coefficients, Compressor, CompressorSettings, Expander, Gate, Limiter, SampleRate,
};
use timeline::{Dynamics, Param, Ticks, VocalMode};

/// The running state of Premiere's Dynamics: each section keeps its own envelope.
#[derive(Default)]
pub(crate) struct DynamicsChain {
    gate: Gate,
    expander: Expander,
    compressor: Compressor,
    limiter: Limiter,
}

impl DynamicsChain {
    pub(crate) fn reset(&mut self) {
        self.gate.reset();
        self.expander.reset();
        self.compressor.reset();
        self.limiter.reset();
    }

    pub(crate) fn process(
        &mut self,
        dynamics: &Dynamics,
        time: Ticks,
        rate: SampleRate,
        samples: &mut [f32],
    ) {
        let value = |param: &Param| param.value_at(time);
        if let Some(gate) = &dynamics.gate {
            let (attack, release) = (value(&gate.attack), value(&gate.release));
            (self.gate).process(value(&gate.threshold), attack, release, rate, samples);
        }
        if let Some(expander) = &dynamics.expander {
            let (threshold, ratio) = (value(&expander.threshold), value(&expander.ratio));
            (self.expander).process(threshold, ratio, EXPANDER_RELEASE_MS, rate, samples);
        }
        if let Some(compressor) = &dynamics.compressor {
            let settings = CompressorSettings {
                threshold_db: value(&compressor.threshold),
                ratio: value(&compressor.ratio),
                attack_ms: value(&compressor.attack),
                release_ms: value(&compressor.release),
                makeup_db: value(&compressor.makeup),
            };
            self.compressor.process(&settings, rate, samples);
        }
        if let Some(limiter) = &dynamics.limiter {
            let (ceiling, release) = (value(&limiter.threshold), value(&limiter.release));
            self.limiter.process(0.0, ceiling, release, rate, samples);
        }
    }
}

/// Premiere's expander has no timing of its own to set; this keeps it from pumping.
const EXPANDER_RELEASE_MS: f32 = 100.0;

/// Vocal Enhancer: three filters, then for voices a gentle compressor to even them out.
#[derive(Default)]
pub(crate) struct VocalChain {
    filters: [Biquad; 3],
    compressor: Compressor,
}

const VOICE_LEVELER: CompressorSettings = CompressorSettings {
    threshold_db: -24.0,
    ratio: 3.0,
    attack_ms: 5.0,
    release_ms: 80.0,
    makeup_db: 3.0,
};

impl VocalChain {
    pub(crate) fn reset(&mut self) {
        self.filters.iter_mut().for_each(Biquad::reset);
        self.compressor.reset();
    }

    pub(crate) fn process(&mut self, mode: VocalMode, rate: SampleRate, samples: &mut [f32]) {
        let shapes = match mode {
            // Rumble out, the boxy low mids down and presence up.
            VocalMode::LowTone => [
                Coefficients::high_pass(rate, 80.0, 0.707),
                Coefficients::peaking(rate, 250.0, 1.0, -3.0),
                Coefficients::peaking(rate, 3_000.0, 1.0, 3.0),
            ],
            // Body up and the harsh upper mids down.
            VocalMode::HighTone => [
                Coefficients::high_pass(rate, 100.0, 0.707),
                Coefficients::peaking(rate, 300.0, 1.0, 2.0),
                Coefficients::peaking(rate, 5_000.0, 1.5, -3.0),
            ],
            // A hollow where speech is most intelligible, so a voice over it comes through.
            VocalMode::Music => [
                Coefficients::peaking(rate, 1_000.0, 1.0, -3.0),
                Coefficients::peaking(rate, 2_500.0, 1.0, -4.0),
                Coefficients::peaking(rate, 4_000.0, 1.0, -2.0),
            ],
        };
        for (filter, shape) in self.filters.iter_mut().zip(shapes) {
            filter.set(shape);
            filter.process(samples);
        }
        if mode != VocalMode::Music {
            self.compressor.process(&VOICE_LEVELER, rate, samples);
        }
    }
}
