use std::f32::consts::FRAC_1_SQRT_2;

use super::{Biquad, Coefficients, Compressor, CompressorSettings};
use crate::SampleRate;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MultibandSettings {
    /// Where the low band hands over to the mid, and the mid to the high.
    pub crossovers_hz: [f32; 2],
    /// The low, mid and high bands' compression.
    pub bands: [CompressorSettings; 3],
}

/// Compresses the lows, mids and highs each on their own, so a booming bass is held down
/// without pumping the voice above it.
///
/// The bands are split with fourth-order Linkwitz-Riley crossovers, whose two sides sum
/// back to a flat response.
#[derive(Debug, Clone, Default)]
pub struct MultibandCompressor {
    below_low: [Biquad; 2],
    above_low: [Biquad; 2],
    below_high: [Biquad; 2],
    above_high: [Biquad; 2],
    /// Turns the low band's phase as the high crossover turns the others', without which
    /// the bands would partly cancel around that crossover.
    low_phase: Biquad,
    compressors: [Compressor; 3],
    low_band: Vec<f32>,
    mid_band: Vec<f32>,
}

impl MultibandCompressor {
    pub fn reset(&mut self) {
        let filters = [
            &mut self.below_low,
            &mut self.above_low,
            &mut self.below_high,
            &mut self.above_high,
        ];
        filters.into_iter().flatten().for_each(Biquad::reset);
        self.low_phase.reset();
        self.compressors.iter_mut().for_each(Compressor::reset);
    }

    pub fn process(&mut self, settings: &MultibandSettings, rate: SampleRate, samples: &mut [f32]) {
        let [first, second] = settings.crossovers_hz;
        let (low_hz, high_hz) = (first.min(second), first.max(second));
        let crossover = |filters: &mut [Biquad; 2], coefficients| {
            filters
                .iter_mut()
                .for_each(|filter| filter.set(coefficients));
        };
        crossover(
            &mut self.below_low,
            Coefficients::low_pass(rate, low_hz, FRAC_1_SQRT_2),
        );
        crossover(
            &mut self.above_low,
            Coefficients::high_pass(rate, low_hz, FRAC_1_SQRT_2),
        );
        crossover(
            &mut self.below_high,
            Coefficients::low_pass(rate, high_hz, FRAC_1_SQRT_2),
        );
        crossover(
            &mut self.above_high,
            Coefficients::high_pass(rate, high_hz, FRAC_1_SQRT_2),
        );
        self.low_phase
            .set(Coefficients::all_pass(rate, high_hz, FRAC_1_SQRT_2));

        self.low_band.clear();
        self.low_band.extend_from_slice(samples);
        self.below_low
            .iter_mut()
            .chain([&mut self.low_phase])
            .for_each(|filter| filter.process(&mut self.low_band));

        self.above_low
            .iter_mut()
            .for_each(|filter| filter.process(samples));
        self.mid_band.clear();
        self.mid_band.extend_from_slice(samples);
        self.below_high
            .iter_mut()
            .for_each(|filter| filter.process(&mut self.mid_band));
        self.above_high
            .iter_mut()
            .for_each(|filter| filter.process(samples));

        let bands = [&mut self.low_band[..], &mut self.mid_band[..], samples];
        for ((band, compressor), settings) in bands
            .into_iter()
            .zip(&mut self.compressors)
            .zip(&settings.bands)
        {
            compressor.process(settings, rate, band);
        }
        for ((sample, low), mid) in samples.iter_mut().zip(&self.low_band).zip(&self.mid_band) {
            *sample += low + mid;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::TAU;

    use super::*;
    use crate::dsp::db_to_amplitude;

    const RATE: SampleRate = SampleRate(48_000);

    fn settings(thresholds_db: [f32; 3]) -> MultibandSettings {
        MultibandSettings {
            // Close crossovers, where the low band's phase matters most.
            crossovers_hz: [300.0, 1_200.0],
            bands: thresholds_db.map(|threshold_db| CompressorSettings {
                threshold_db,
                ratio: 8.0,
                attack_ms: 1.0,
                release_ms: 50.0,
                makeup_db: 0.0,
            }),
        }
    }

    /// The level of a steady tone through the compressor, as a ratio of its input level.
    fn gain(settings: &MultibandSettings, frequency: f32, amplitude: f32) -> f32 {
        let mut samples: Vec<f32> = (0..RATE.0)
            .flat_map(|frame| {
                [amplitude * (TAU * frequency * frame as f32 / RATE.0 as f32).sin(); 2]
            })
            .collect();
        MultibandCompressor::default().process(settings, RATE, &mut samples);
        // The last tenth of a second holds whole cycles of every tone tested.
        let settled = &samples[samples.len() - 9_600..];
        let power =
            settled.iter().map(|sample| sample * sample).sum::<f32>() / settled.len() as f32;
        power.sqrt() * 2.0_f32.sqrt() / amplitude
    }

    #[test]
    fn bands_under_their_thresholds_sum_back_to_the_input_level() {
        let untouched = settings([0.0; 3]);
        for frequency in [50.0, 300.0, 600.0, 1_200.0, 8_000.0] {
            let gain = gain(&untouched, frequency, 0.5);
            assert!((gain - 1.0).abs() < 0.01, "{frequency} Hz: {gain}");
        }
    }

    #[test]
    fn each_band_answers_only_to_its_own_threshold() {
        let lows_held = settings([-30.0, 0.0, 0.0]);
        let loud = db_to_amplitude(-6.0);
        assert!(gain(&lows_held, 60.0, loud) < db_to_amplitude(-15.0));
        let high = gain(&lows_held, 6_000.0, loud);
        assert!((high - 1.0).abs() < 0.01, "{high}");
    }
}
