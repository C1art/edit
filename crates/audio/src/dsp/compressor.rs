use super::db_to_amplitude;
use crate::SampleRate;

/// Quieter than any level the compressor needs to tell apart.
const SILENCE_DB: f32 = -120.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompressorSettings {
    pub threshold_db: f32,
    /// How many decibels over the threshold become one; at least 1.
    pub ratio: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub makeup_db: f32,
}

/// Turns loud passages down, both sides together so the stereo image holds still.
#[derive(Debug, Clone, Copy, Default)]
pub struct Compressor {
    /// The gain reduction in decibels, smoothed over the attack and release.
    reduction_db: f32,
}

impl Compressor {
    pub fn reset(&mut self) {
        self.reduction_db = 0.0;
    }

    pub fn process(
        &mut self,
        settings: &CompressorSettings,
        rate: SampleRate,
        samples: &mut [f32],
    ) {
        let smoothing = |ms: f32| (-1.0 / (ms.max(0.01) * 0.001 * rate.0 as f32)).exp();
        let (attack, release) = (
            smoothing(settings.attack_ms),
            smoothing(settings.release_ms),
        );
        let slope = 1.0 - 1.0 / settings.ratio.max(1.0);
        for frame in samples.as_chunks_mut::<2>().0 {
            let peak = frame[0].abs().max(frame[1].abs());
            let level_db = if peak > 0.0 {
                (20.0 * peak.log10()).max(SILENCE_DB)
            } else {
                SILENCE_DB
            };
            let target = (level_db - settings.threshold_db).max(0.0) * slope;
            let coefficient = if target > self.reduction_db {
                attack
            } else {
                release
            };
            self.reduction_db = target + coefficient * (self.reduction_db - target);
            let gain = db_to_amplitude(settings.makeup_db - self.reduction_db);
            frame.iter_mut().for_each(|sample| *sample *= gain);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_steady_level_over_the_threshold_settles_at_the_ratio() {
        let settings = CompressorSettings {
            threshold_db: -20.0,
            ratio: 4.0,
            attack_ms: 1.0,
            release_ms: 50.0,
            makeup_db: 3.0,
        };
        let mut compressor = Compressor::default();
        // A constant -8 dBFS is 12 dB over, so 9 dB comes off before the makeup.
        let level = db_to_amplitude(-8.0);
        let mut samples = vec![level; 9_600];
        compressor.process(&settings, SampleRate(48_000), &mut samples);
        let out_db = 20.0 * samples[9_598].log10();
        assert!((out_db - (-8.0 - 9.0 + 3.0)).abs() < 0.01, "{out_db}");
    }
}
