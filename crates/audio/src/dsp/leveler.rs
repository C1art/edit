use super::db_to_amplitude;
use crate::SampleRate;

/// Long enough to follow phrases rather than syllables.
const LEVEL_SECONDS: f32 = 0.4;
const GAIN_SECONDS: f32 = 0.3;
/// The most the leveler raises or lowers, at full amount.
const MAX_GAIN_DB: f32 = 24.0;
const SILENCE_DB: f32 = -120.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LevelerSettings {
    pub target_db: f32,
    /// 0 to 1: how much of the way to the target each phrase is brought.
    pub amount: f32,
    /// Sound quieter than this is taken for pauses, whose gain holds rather than rising
    /// to lift the noise between words.
    pub noise_threshold_db: f32,
}

/// Rides the level of a voice toward a target, both sides together, as Premiere's Speech
/// Volume Leveler does.
#[derive(Debug, Clone, Copy, Default)]
pub struct SpeechLeveler {
    /// Mean power, smoothed over [`LEVEL_SECONDS`].
    power: f32,
    gain_db: f32,
}

impl SpeechLeveler {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn process(&mut self, settings: &LevelerSettings, rate: SampleRate, samples: &mut [f32]) {
        let follow = |seconds: f32| 1.0 - (-1.0 / (seconds * rate.0 as f32)).exp();
        let (level_follow, gain_follow) = (follow(LEVEL_SECONDS), follow(GAIN_SECONDS));
        let reach = settings.amount.clamp(0.0, 1.0);
        for frame in samples.as_chunks_mut::<2>().0 {
            let [left, right] = *frame;
            let power = (left * left + right * right) / 2.0;
            self.power += (power - self.power) * level_follow;
            let level_db = (10.0 * self.power.log10()).max(SILENCE_DB);
            if level_db >= settings.noise_threshold_db {
                let wanted = ((settings.target_db - level_db) * reach)
                    .clamp(-MAX_GAIN_DB * reach, MAX_GAIN_DB * reach);
                self.gain_db += (wanted - self.gain_db) * gain_follow;
            }
            let gain = db_to_amplitude(self.gain_db);
            frame.iter_mut().for_each(|sample| *sample *= gain);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: SampleRate = SampleRate(48_000);

    fn tone(level_db: f32, seconds: f32) -> Vec<f32> {
        let peak = db_to_amplitude(level_db) * std::f32::consts::SQRT_2;
        let frames = (seconds * RATE.0 as f32) as usize;
        (0..frames)
            .flat_map(|frame| {
                let sample = peak * (frame as f32 * 0.05).sin();
                [sample, sample]
            })
            .collect()
    }

    fn settled_db(leveler: &mut SpeechLeveler, mut samples: Vec<f32>) -> f32 {
        let settings = LevelerSettings {
            target_db: -18.0,
            amount: 1.0,
            noise_threshold_db: -50.0,
        };
        leveler.process(&settings, RATE, &mut samples);
        let tail = &samples[samples.len() / 2..];
        let power = tail.iter().map(|sample| sample * sample).sum::<f32>() / tail.len() as f32;
        10.0 * power.log10()
    }

    #[test]
    fn quiet_and_loud_speech_are_brought_to_the_target_but_pauses_are_not_raised() {
        let mut leveler = SpeechLeveler::default();
        assert!((settled_db(&mut leveler, tone(-34.0, 4.0)) + 18.0).abs() < 1.5);
        leveler.reset();
        assert!((settled_db(&mut leveler, tone(-6.0, 4.0)) + 18.0).abs() < 1.5);
        leveler.reset();
        assert!(settled_db(&mut leveler, tone(-60.0, 4.0)) < -59.0);
    }
}
