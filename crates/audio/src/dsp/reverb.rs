use crate::SampleRate;

/// Freeverb's comb and allpass lengths in samples at 44.1 kHz, chosen to share no
/// common factors so their echoes never line up into a pitch.
const COMB_TUNING: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASS_TUNING: [usize; 4] = [556, 441, 341, 225];
/// The right side's lines are this much longer, which widens the image.
const STEREO_SPREAD: usize = 23;
const TUNING_RATE: f32 = 44_100.0;
/// Eight combs summed would clip; this brings the input down to suit.
const INPUT_GAIN: f32 = 0.015;
/// Brings the quiet wet signal back up to about the dry level.
const WET_GAIN: f32 = 3.0;

#[derive(Debug, Clone)]
struct Comb {
    line: Vec<f32>,
    index: usize,
    damped: f32,
}

impl Comb {
    fn process(&mut self, input: f32, feedback: f32, damping: f32) -> f32 {
        let output = self.line[self.index];
        self.damped = output * (1.0 - damping) + self.damped * damping;
        self.line[self.index] = input + self.damped * feedback;
        self.index = (self.index + 1) % self.line.len();
        output
    }
}

#[derive(Debug, Clone)]
struct Allpass {
    line: Vec<f32>,
    index: usize,
}

impl Allpass {
    fn process(&mut self, input: f32) -> f32 {
        let delayed = self.line[self.index];
        self.line[self.index] = input + delayed * 0.5;
        self.index = (self.index + 1) % self.line.len();
        delayed - input
    }
}

/// A room's reflections, after Jezar's Freeverb.
#[derive(Debug, Clone)]
pub struct Reverb {
    combs: [Vec<Comb>; 2],
    allpasses: [Vec<Allpass>; 2],
}

impl Reverb {
    pub fn new(rate: SampleRate) -> Self {
        let scale = rate.0 as f32 / TUNING_RATE;
        let length = |tuning: usize, spread: usize| {
            (((tuning + spread) as f32 * scale).round() as usize).max(1)
        };
        let side = |spread: usize| {
            let combs = COMB_TUNING
                .iter()
                .map(|&tuning| Comb {
                    line: vec![0.0; length(tuning, spread)],
                    index: 0,
                    damped: 0.0,
                })
                .collect();
            let allpasses = ALLPASS_TUNING
                .iter()
                .map(|&tuning| Allpass {
                    line: vec![0.0; length(tuning, spread)],
                    index: 0,
                })
                .collect();
            (combs, allpasses)
        };
        let (left_combs, left_allpasses) = side(0);
        let (right_combs, right_allpasses) = side(STEREO_SPREAD);
        Self {
            combs: [left_combs, right_combs],
            allpasses: [left_allpasses, right_allpasses],
        }
    }

    pub fn reset(&mut self) {
        for comb in self.combs.iter_mut().flatten() {
            comb.line.fill(0.0);
            comb.damped = 0.0;
        }
        for allpass in self.allpasses.iter_mut().flatten() {
            allpass.line.fill(0.0);
        }
    }

    /// Mixes reflections into interleaved stereo `samples`; `size`, `damping` and `mix`
    /// are fractions, a larger room ringing longer and more damping dulling the tail.
    pub fn process(&mut self, size: f32, damping: f32, mix: f32, samples: &mut [f32]) {
        let feedback = 0.7 + 0.28 * size.clamp(0.0, 1.0);
        let damping = 0.4 * damping.clamp(0.0, 1.0);
        let mix = mix.clamp(0.0, 1.0);
        for frame in samples.as_chunks_mut::<2>().0 {
            let input = (frame[0] + frame[1]) * INPUT_GAIN;
            for (side, sample) in frame.iter_mut().enumerate() {
                let mut wet: f32 = self.combs[side]
                    .iter_mut()
                    .map(|comb| comb.process(input, feedback, damping))
                    .sum();
                for allpass in &mut self.allpasses[side] {
                    wet = allpass.process(wet);
                }
                *sample = *sample * (1.0 - mix) + wet * WET_GAIN * mix;
            }
        }
    }
}
