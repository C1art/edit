use super::db_to_amplitude;
use super::dynamics::{smoothing, LEVEL_HOLD_MS};
use crate::SampleRate;

const SILENCE_DB: f32 = -120.0;
/// The most a downward expander takes off, so that it fades rather than cuts.
const MAX_REDUCTION_DB: f32 = 60.0;
/// Opening fast keeps the starts of words; the release sets how it closes.
const ATTACK_MS: f32 = 1.0;

/// Pushes passages under a threshold further down: each decibel under becomes `ratio`,
/// both sides together.
#[derive(Debug, Clone, Copy, Default)]
pub struct Expander {
    reduction_db: f32,
    level: f32,
}

impl Expander {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn process(
        &mut self,
        threshold_db: f32,
        ratio: f32,
        release_ms: f32,
        rate: SampleRate,
        samples: &mut [f32],
    ) {
        let (attack, release) = (smoothing(ATTACK_MS, rate), smoothing(release_ms, rate));
        let hold = smoothing(LEVEL_HOLD_MS, rate);
        let slope = ratio.max(1.0) - 1.0;
        for frame in samples.as_chunks_mut::<2>().0 {
            // Held through the dips of the waveform, so a tone under the threshold
            // isn't turned down at every zero crossing.
            self.level = super::flush(frame[0].abs().max(frame[1].abs()).max(self.level * hold));
            let level_db = match self.level > 0.0 {
                true => (20.0 * self.level.log10()).max(SILENCE_DB),
                false => SILENCE_DB,
            };
            let target = ((threshold_db - level_db).max(0.0) * slope).min(MAX_REDUCTION_DB);
            let coefficient = if target < self.reduction_db {
                attack
            } else {
                release
            };
            self.reduction_db = target + coefficient * (self.reduction_db - target);
            let gain = db_to_amplitude(-self.reduction_db);
            frame.iter_mut().for_each(|sample| *sample *= gain);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_under_the_threshold_fall_by_the_ratio_and_those_over_pass() {
        let settle = |level_db: f32| {
            let mut samples = vec![db_to_amplitude(level_db); 19_200];
            Expander::default().process(-40.0, 2.0, 10.0, SampleRate(48_000), &mut samples);
            20.0 * samples[19_198].log10()
        };
        // 10 dB under becomes 20 dB under.
        assert!((settle(-50.0) - -60.0).abs() < 0.01);
        assert!((settle(-20.0) - -20.0).abs() < 0.01);
    }

    #[test]
    fn a_tone_over_the_threshold_passes_whole_through_its_zero_crossings() {
        let amplitude = db_to_amplitude(-20.0);
        let tone: Vec<f32> = (0..9_600)
            .flat_map(|frame| {
                let sample =
                    amplitude * (frame as f32 * 100.0 * std::f32::consts::TAU / 48_000.0).sin();
                [sample, sample]
            })
            .collect();
        let mut samples = tone.clone();
        Expander::default().process(-30.0, 4.0, 5.0, SampleRate(48_000), &mut samples);
        // Past the onset, where it rises from silence.
        let worst = samples
            .iter()
            .zip(&tone)
            .skip(2 * 2_400)
            .map(|(out, input)| (out - input).abs())
            .fold(0.0, f32::max);
        assert!(worst < amplitude * 0.01, "{worst}");
    }
}
