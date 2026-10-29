use audio::{distort, Biquad, Coefficients, Compressor, CompressorSettings, SampleRate};
use timeline::{AmpModel, GuitarFilter};

/// The Guitar Suite's compressor at full amount.
const DEEPEST_THRESHOLD_DB: f32 = -30.0;
const STEEPEST_RATIO: f32 = 8.0;
/// Gives back about half of what full compression takes away.
const MOST_MAKEUP_DB: f32 = 12.0;
const MOST_DRIVE_DB: f32 = 36.0;
/// Clipping is never transparent, so the first tenth of the distortion fades it in
/// rather than the level jumping as it turns on.
const DISTORTION_FADE: f32 = 10.0;
const FLAT_Q: f32 = std::f32::consts::FRAC_1_SQRT_2;
const MOST_RESONANCE_Q: f32 = 12.0;
/// Frames mixed at a time, so the dry sound fits on the stack.
const BLOCK_FRAMES: usize = 256;

pub(crate) struct GuitarSettings {
    pub(crate) compression: f32,
    pub(crate) distortion: f32,
    pub(crate) amp: AmpModel,
    pub(crate) filter: GuitarFilter,
    pub(crate) frequency_hz: f32,
    pub(crate) resonance: f32,
    pub(crate) mix: f32,
}

/// Guitar Suite: compressor, distortion, an amp's cabinet and a resonant filter.
#[derive(Default)]
pub(crate) struct GuitarChain {
    compressor: Compressor,
    cabinet: [Biquad; 3],
    filter: Biquad,
}

impl GuitarChain {
    pub(crate) fn reset(&mut self) {
        self.compressor.reset();
        self.cabinet.iter_mut().for_each(Biquad::reset);
        self.filter.reset();
    }

    pub(crate) fn process(
        &mut self,
        settings: &GuitarSettings,
        rate: SampleRate,
        samples: &mut [f32],
    ) {
        let compression = settings.compression.clamp(0.0, 1.0);
        let squeeze = CompressorSettings {
            threshold_db: DEEPEST_THRESHOLD_DB * compression,
            ratio: 1.0 + (STEEPEST_RATIO - 1.0) * compression,
            attack_ms: 5.0,
            release_ms: 100.0,
            makeup_db: MOST_MAKEUP_DB * compression,
        };
        let distortion = settings.distortion.clamp(0.0, 1.0);
        let cabinet = cabinet(settings.amp, rate);
        let q = FLAT_Q + (MOST_RESONANCE_Q - FLAT_Q) * settings.resonance.clamp(0.0, 1.0);
        let filter = match settings.filter {
            GuitarFilter::None => None,
            GuitarFilter::LowPass => Some(Coefficients::low_pass(rate, settings.frequency_hz, q)),
            GuitarFilter::BandPass => Some(Coefficients::band_pass(rate, settings.frequency_hz, q)),
            GuitarFilter::HighPass => Some(Coefficients::high_pass(rate, settings.frequency_hz, q)),
        };
        let mix = settings.mix.clamp(0.0, 1.0);
        for block in samples.chunks_mut(BLOCK_FRAMES * 2) {
            let mut dry = [0.0; BLOCK_FRAMES * 2];
            dry[..block.len()].copy_from_slice(block);
            if compression > 0.0 {
                self.compressor.process(&squeeze, rate, block);
            }
            let fade = (distortion * DISTORTION_FADE).min(1.0);
            distort(MOST_DRIVE_DB * distortion, fade, block);
            if let Some(cabinet) = cabinet {
                for (stage, shape) in self.cabinet.iter_mut().zip(cabinet) {
                    stage.set(shape);
                    stage.process(block);
                }
            }
            if let Some(filter) = filter {
                self.filter.set(filter);
                self.filter.process(block);
            }
            for (wet, dry) in block.iter_mut().zip(dry) {
                *wet = dry + (*wet - dry) * mix;
            }
        }
    }
}

/// The amp's tone and its speaker's narrow range, as a highpass, a body bell and a
/// lowpass.
fn cabinet(amp: AmpModel, rate: SampleRate) -> Option<[Coefficients; 3]> {
    let (low_hz, body_hz, body_db, high_hz) = match amp {
        AmpModel::None => return None,
        AmpModel::Small => (150.0, 1_500.0, 4.0, 3_500.0),
        AmpModel::Combo => (90.0, 800.0, 3.0, 5_000.0),
        AmpModel::Stack => (70.0, 500.0, 5.0, 6_000.0),
        AmpModel::Bass => (40.0, 100.0, 6.0, 2_500.0),
    };
    Some([
        Coefficients::high_pass(rate, low_hz, FLAT_Q),
        Coefficients::peaking(rate, body_hz, 1.0, body_db),
        Coefficients::low_pass(rate, high_hz, FLAT_Q),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dry_sound_is_kept_in_step_across_blocks_and_distortion_flattens_peaks() {
        let rate = SampleRate(48_000);
        let tone: Vec<f32> = (0..1_000)
            .flat_map(|frame| [(frame as f32 * 0.05).sin() * 0.9; 2])
            .collect();
        let mut settings = GuitarSettings {
            compression: 0.5,
            distortion: 1.0,
            amp: AmpModel::Combo,
            filter: GuitarFilter::LowPass,
            frequency_hz: 2_000.0,
            resonance: 0.5,
            mix: 0.0,
        };
        let mut samples = tone.clone();
        GuitarChain::default().process(&settings, rate, &mut samples);
        assert_eq!(samples, tone);
        let (amp, filter) = (AmpModel::None, GuitarFilter::None);
        settings = GuitarSettings {
            compression: 0.0,
            amp,
            filter,
            mix: 1.0,
            ..settings
        };
        GuitarChain::default().process(&settings, rate, &mut samples);
        let mean_square = samples.iter().map(|sample| sample * sample).sum::<f32>();
        let crest = samples
            .iter()
            .fold(0.0_f32, |peak, sample| peak.max(sample.abs()))
            / (mean_square / samples.len() as f32).sqrt();
        // A sine's is the square root of two; a square wave's is one.
        assert!(crest < 1.1);
    }
}
