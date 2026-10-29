/// How fast the measure of a channel's ordinary sample-to-sample roughness follows it.
const ROUGHNESS_SMOOTHING: f32 = 0.999;
/// Roughness below this is silence, where no sample counts as a click.
const ROUGHNESS_FLOOR: f32 = 1e-4;

/// Finds single-sample clicks, which stand out from the line through their neighbors
/// far more than the sound around them does, and puts them back on that line. A click
/// also pulls its neighbors off their lines by half as much, so only the sample that
/// stands out most among its neighbors counts. The sound comes out two samples late, as
/// each sample is judged against the two after it.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClickRemover {
    sides: [Side; 2],
}

#[derive(Debug, Clone, Copy, Default)]
struct Side {
    /// The three samples before the one coming in, the earliest first.
    history: [f32; 3],
    /// How far the earliest of them stood out.
    earlier_deviation: f32,
    roughness: f32,
}

impl ClickRemover {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Repairs interleaved stereo `samples`; a sample counts as a click once it stands
    /// out `sensitivity` times further than usual.
    pub fn process(&mut self, sensitivity: f32, samples: &mut [f32]) {
        for frame in samples.as_chunks_mut::<2>().0 {
            for (side, sample) in self.sides.iter_mut().zip(frame) {
                *sample = side.next(sensitivity, *sample);
            }
        }
    }
}

impl Side {
    fn next(&mut self, sensitivity: f32, incoming: f32) -> f32 {
        let [before, judged, after] = self.history;
        let line = (before + after) / 2.0;
        let deviation = (judged - line).abs();
        let later_deviation = (after - (judged + incoming) / 2.0).abs();
        let usual = self.roughness.max(ROUGHNESS_FLOOR);
        let click = deviation > sensitivity * usual
            && deviation >= self.earlier_deviation
            && deviation >= later_deviation;
        let repaired = if click {
            line
        } else {
            self.roughness =
                ROUGHNESS_SMOOTHING * self.roughness + (1.0 - ROUGHNESS_SMOOTHING) * deviation;
            judged
        };
        self.history = [judged, after, incoming];
        self.earlier_deviation = deviation;
        repaired
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lone_spike_is_smoothed_away_and_the_tone_around_it_kept() {
        let tone: Vec<f32> = (0..9_600)
            .flat_map(|index| [(index as f32 * 0.05).sin() * 0.5; 2])
            .collect();
        let mut clicked = tone.clone();
        clicked[2 * 6_000] += 0.8;
        ClickRemover::default().process(8.0, &mut clicked);
        // Two frames late: output frame n is input frame n - 2.
        let error = |frame: usize| (clicked[2 * frame] - tone[2 * (frame - 2)]).abs();
        assert!((5_990..6_010).all(|frame| error(frame) < 0.01));
        assert!(error(5_000) < 1e-6);
        assert!(error(7_000) < 1e-6);
    }
}
