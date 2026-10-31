use super::db_to_amplitude;
use crate::SampleRate;

/// How fast a level follower lets go of a peak, so a gate holds through the dips of
/// a waveform rather than closing at every zero crossing.
pub(super) const LEVEL_HOLD_MS: f32 = 20.0;

/// The per-sample coefficient of a one-pole smoother with a time constant of `ms`.
pub(super) fn smoothing(ms: f32, rate: SampleRate) -> f32 {
    (-1.0 / (ms.max(0.01) * 0.001 * rate.0 as f32)).exp()
}

/// Keeps peaks under a ceiling, turning down at once and recovering over the release.
#[derive(Debug, Clone, Copy)]
pub struct Limiter {
    gain: f32,
}

impl Default for Limiter {
    fn default() -> Self {
        Self { gain: 1.0 }
    }
}

impl Limiter {
    pub fn reset(&mut self) {
        self.gain = 1.0;
    }

    /// Lifts interleaved stereo `samples` by `boost_db`, then holds both sides together
    /// under `ceiling_db`.
    pub fn process(
        &mut self,
        boost_db: f32,
        ceiling_db: f32,
        release_ms: f32,
        rate: SampleRate,
        samples: &mut [f32],
    ) {
        let boost = db_to_amplitude(boost_db);
        let ceiling = db_to_amplitude(ceiling_db);
        let release = smoothing(release_ms, rate);
        for frame in samples.as_chunks_mut::<2>().0 {
            let peak = frame[0].abs().max(frame[1].abs()) * boost;
            let allowed = if peak > ceiling { ceiling / peak } else { 1.0 };
            self.gain = if allowed < self.gain {
                allowed
            } else {
                allowed + release * (self.gain - allowed)
            };
            let gain = boost * self.gain;
            frame.iter_mut().for_each(|sample| *sample *= gain);
        }
    }
}

/// Silences passages quieter than a threshold, such as room noise between words.
#[derive(Debug, Clone, Copy)]
pub struct Gate {
    level: f32,
    gain: f32,
}

impl Default for Gate {
    fn default() -> Self {
        // Closed, so a clip that starts quiet starts silent.
        Self {
            level: 0.0,
            gain: 0.0,
        }
    }
}

impl Gate {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Opens over `attack_ms` when interleaved stereo `samples` reach `threshold_db`,
    /// and closes over `release_ms` once they fall below it.
    pub fn process(
        &mut self,
        threshold_db: f32,
        attack_ms: f32,
        release_ms: f32,
        rate: SampleRate,
        samples: &mut [f32],
    ) {
        let threshold = db_to_amplitude(threshold_db);
        let hold = smoothing(LEVEL_HOLD_MS, rate);
        let (attack, release) = (smoothing(attack_ms, rate), smoothing(release_ms, rate));
        for frame in samples.as_chunks_mut::<2>().0 {
            let peak = frame[0].abs().max(frame[1].abs());
            self.level = super::flush(peak.max(self.level * hold));
            let open = if self.level >= threshold { 1.0 } else { 0.0 };
            let coefficient = if open > self.gain { attack } else { release };
            self.gain = super::flush(open + coefficient * (self.gain - open));
            frame.iter_mut().for_each(|sample| *sample *= self.gain);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_limiter_never_lets_a_boosted_peak_past_the_ceiling() {
        let mut limiter = Limiter::default();
        let mut samples: Vec<f32> = (0..4_800)
            .map(|index| (index as f32 * 0.05).sin() * 0.9)
            .collect();
        limiter.process(12.0, -1.0, 50.0, SampleRate(48_000), &mut samples);
        let ceiling = db_to_amplitude(-1.0);
        let loudest = samples.iter().fold(0.0_f32, |max, s| max.max(s.abs()));
        assert!(loudest <= ceiling + 1e-6, "{loudest} > {ceiling}");
        assert!(
            loudest > ceiling * 0.99,
            "{loudest} pulled well under the ceiling"
        );
    }

    #[test]
    fn the_gate_passes_a_tone_over_the_threshold_and_mutes_one_under_it() {
        let rate = SampleRate(48_000);
        let tone = |amplitude: f32| -> Vec<f32> {
            (0..9_600)
                .map(|index| (index as f32 * 0.06).sin() * amplitude)
                .collect()
        };
        let mut loud = tone(0.5);
        Gate::default().process(-30.0, 1.0, 50.0, rate, &mut loud);
        let mut quiet = tone(0.01);
        Gate::default().process(-30.0, 1.0, 50.0, rate, &mut quiet);
        let tail_peak = |samples: &[f32]| {
            samples[7_200..]
                .iter()
                .fold(0.0_f32, |max, s| max.max(s.abs()))
        };
        assert!((tail_peak(&loud) - 0.5).abs() < 0.01);
        assert!(tail_peak(&quiet) < 1e-4);
    }
}
