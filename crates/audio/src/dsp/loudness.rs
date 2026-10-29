use std::f64::consts::PI;

use super::{Biquad, Coefficients};
use crate::SampleRate;

/// Gating blocks are 400 ms long and start every 100 ms, so each spans four steps.
const STEP_SECONDS: f64 = 0.1;
const STEPS_PER_BLOCK: usize = 4;
const ABSOLUTE_GATE_LUFS: f64 = -70.0;
/// Blocks this far below the loudness of the blocks above the absolute gate are left
/// out, so pauses don't drag the result down.
const RELATIVE_GATE_LU: f64 = -10.0;
/// Offsets the K-weighting's gain at 1 kHz, so a full-scale sine reads its level.
const LOUDNESS_OFFSET: f64 = -0.691;

/// Integrated loudness after EBU R128 / ITU-R BS.1770 and sample peak of interleaved
/// stereo pushed through it.
#[derive(Debug, Clone)]
pub struct LoudnessMeter {
    shelf: Biquad,
    high_pass: Biquad,
    weighted: Vec<f32>,
    step_frames: usize,
    /// Sum of squares over both sides of the step being filled, and its frames so far.
    step_energy: f64,
    step_filled: usize,
    /// The latest steps' energies, oldest first, at most one block's worth.
    recent: Vec<f64>,
    /// Mean square of every whole block.
    blocks: Vec<f64>,
    peak: f32,
}

impl LoudnessMeter {
    pub fn new(rate: SampleRate) -> Self {
        let mut shelf = Biquad::default();
        shelf.set(k_shelf(rate));
        let mut high_pass = Biquad::default();
        high_pass.set(k_high_pass(rate));
        Self {
            shelf,
            high_pass,
            weighted: Vec::new(),
            step_frames: ((f64::from(rate.0) * STEP_SECONDS).round() as usize).max(1),
            step_energy: 0.0,
            step_filled: 0,
            recent: Vec::with_capacity(STEPS_PER_BLOCK),
            blocks: Vec::new(),
            peak: 0.0,
        }
    }

    pub fn push(&mut self, samples: &[f32]) {
        self.peak = samples
            .iter()
            .fold(self.peak, |peak, sample| peak.max(sample.abs()));
        let mut weighted = std::mem::take(&mut self.weighted);
        weighted.clear();
        weighted.extend_from_slice(samples);
        self.shelf.process(&mut weighted);
        self.high_pass.process(&mut weighted);
        for frame in weighted.as_chunks::<2>().0 {
            self.step_energy += frame
                .iter()
                .map(|&sample| f64::from(sample) * f64::from(sample))
                .sum::<f64>();
            self.step_filled += 1;
            if self.step_filled == self.step_frames {
                self.finish_step();
            }
        }
        self.weighted = weighted;
    }

    fn finish_step(&mut self) {
        if self.recent.len() == STEPS_PER_BLOCK {
            self.recent.remove(0);
        }
        self.recent.push(self.step_energy);
        self.step_energy = 0.0;
        self.step_filled = 0;
        if self.recent.len() == STEPS_PER_BLOCK {
            let frames = (self.step_frames * STEPS_PER_BLOCK) as f64;
            self.blocks.push(self.recent.iter().sum::<f64>() / frames);
        }
    }

    /// The gated loudness of everything pushed, in LUFS, or `None` if nothing rose above
    /// the absolute gate.
    pub fn integrated(&self) -> Option<f32> {
        let absolute = gated_mean(&self.blocks, power(ABSOLUTE_GATE_LUFS))?;
        let relative = power(loudness(absolute) + RELATIVE_GATE_LU);
        let mean = gated_mean(&self.blocks, relative.max(power(ABSOLUTE_GATE_LUFS)))?;
        Some(loudness(mean) as f32)
    }

    /// The largest sample magnitude pushed, as a linear amplitude.
    pub fn peak(&self) -> f32 {
        self.peak
    }
}

fn gated_mean(blocks: &[f64], threshold: f64) -> Option<f64> {
    let (sum, count) = blocks
        .iter()
        .filter(|&&block| block > threshold)
        .fold((0.0, 0_usize), |(sum, count), block| {
            (sum + block, count + 1)
        });
    (count > 0).then(|| sum / count as f64)
}

fn loudness(power: f64) -> f64 {
    LOUDNESS_OFFSET + 10.0 * power.log10()
}

fn power(loudness: f64) -> f64 {
    10.0_f64.powf((loudness - LOUDNESS_OFFSET) / 10.0)
}

/// The K-weighting's first stage, a shelf modelling the head, with BS.1770's analogue
/// prototype mapped to `rate` so rates other than 48 kHz weigh the same.
fn k_shelf(rate: SampleRate) -> Coefficients {
    const FREQUENCY: f64 = 1_681.974_450_955_533;
    const GAIN_DB: f64 = 3.999_843_853_973_347;
    const Q: f64 = 0.707_175_236_955_419_6;
    let k = (PI * FREQUENCY / f64::from(rate.0)).tan();
    let high = 10.0_f64.powf(GAIN_DB / 20.0);
    let band = high.powf(0.499_666_774_154_541_6);
    Coefficients::normalised(
        [
            high + band * k / Q + k * k,
            2.0 * (k * k - high),
            high - band * k / Q + k * k,
        ],
        [
            1.0 + k / Q + k * k,
            2.0 * (k * k - 1.0),
            1.0 - k / Q + k * k,
        ],
    )
}

/// The K-weighting's second stage, a high pass (RLB weighting).
fn k_high_pass(rate: SampleRate) -> Coefficients {
    const FREQUENCY: f64 = 38.135_470_876_024_44;
    const Q: f64 = 0.500_327_037_323_877_3;
    let k = (PI * FREQUENCY / f64::from(rate.0)).tan();
    let a0 = 1.0 + k / Q + k * k;
    Coefficients::normalised(
        [a0, -2.0 * a0, a0],
        [a0, 2.0 * (k * k - 1.0), 1.0 - k / Q + k * k],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: SampleRate = SampleRate(48_000);

    fn sine(level_db: f32, seconds: f32) -> Vec<f32> {
        let amplitude = 10.0_f32.powf(level_db / 20.0);
        let frames = (RATE.0 as f32 * seconds) as usize;
        (0..frames)
            .flat_map(|frame| {
                let phase = frame as f32 / RATE.0 as f32 * 1_000.0 * std::f32::consts::TAU;
                let sample = amplitude * phase.sin();
                [sample, sample]
            })
            .collect()
    }

    // EBU Tech 3341, cases 1 and 3.
    #[test]
    fn a_stereo_sine_reads_its_level_and_quiet_passages_are_gated_out() {
        let mut meter = LoudnessMeter::new(RATE);
        for chunk in sine(-23.0, 20.0).chunks(2_000) {
            meter.push(chunk);
        }
        let integrated = meter.integrated().unwrap();
        assert!((integrated + 23.0).abs() < 0.1, "{integrated}");
        assert!((meter.peak() - 10.0_f32.powf(-23.0 / 20.0)).abs() < 1e-3);

        let mut meter = LoudnessMeter::new(RATE);
        meter.push(&sine(-36.0, 10.0));
        meter.push(&sine(-23.0, 60.0));
        meter.push(&sine(-36.0, 10.0));
        meter.push(&vec![0.0; RATE.0 as usize * 2 * 5]);
        let integrated = meter.integrated().unwrap();
        assert!((integrated + 23.0).abs() < 0.1, "{integrated}");
    }

    #[test]
    fn silence_has_no_loudness() {
        let mut meter = LoudnessMeter::new(RATE);
        meter.push(&vec![0.0; RATE.0 as usize * 2]);
        assert_eq!(meter.integrated(), None);
    }
}
