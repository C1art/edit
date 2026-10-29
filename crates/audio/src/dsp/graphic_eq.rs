use super::{Biquad, Coefficients};
use crate::SampleRate;

/// The centres of the ten-band equalizer's bands, an octave apart.
pub const GRAPHIC_BANDS_HZ: [f32; 10] = [
    31.0, 63.0, 125.0, 250.0, 500.0, 1_000.0, 2_000.0, 4_000.0, 8_000.0, 16_000.0,
];
/// Half an octave apart, as Premiere's twenty-band equalizer is.
pub const GRAPHIC_BANDS_20_HZ: [f32; 20] = [
    31.0, 44.0, 63.0, 88.0, 125.0, 180.0, 250.0, 350.0, 500.0, 700.0, 1_000.0, 1_400.0, 2_000.0,
    2_800.0, 4_000.0, 5_600.0, 8_000.0, 11_000.0, 16_000.0, 22_000.0,
];
/// The standard third-octave centres from 25 Hz to 20 kHz.
pub const GRAPHIC_BANDS_30_HZ: [f32; 30] = [
    25.0, 31.5, 40.0, 50.0, 63.0, 80.0, 100.0, 125.0, 160.0, 200.0, 250.0, 315.0, 400.0, 500.0,
    630.0, 800.0, 1_000.0, 1_250.0, 1_600.0, 2_000.0, 2_500.0, 3_150.0, 4_000.0, 5_000.0, 6_300.0,
    8_000.0, 10_000.0, 12_500.0, 16_000.0, 20_000.0,
];
/// Bands this close to half the sample rate are moved below it, where a bell can sit.
const HIGHEST_FRACTION_OF_RATE: f32 = 0.45;

/// A bell at each of `N` centres, each boosting or cutting by its own gain and spanning
/// the distance to its neighbours, so a row of equal gains is near flat between them.
#[derive(Debug, Clone)]
pub struct GraphicEqualizer<const N: usize> {
    centers: [f32; N],
    q: f32,
    bands: [Biquad; N],
}

pub type GraphicEqualizer10 = GraphicEqualizer<10>;
pub type GraphicEqualizer20 = GraphicEqualizer<20>;
pub type GraphicEqualizer30 = GraphicEqualizer<30>;

impl<const N: usize> GraphicEqualizer<N> {
    /// Bands `octaves` apart at `centers`.
    fn spaced(centers: [f32; N], octaves: f32) -> Self {
        let width = 2.0_f32.powf(octaves);
        Self {
            centers,
            q: width.sqrt() / (width - 1.0),
            bands: [Biquad::default(); N],
        }
    }

    pub fn reset(&mut self) {
        self.bands.iter_mut().for_each(Biquad::reset);
    }

    pub fn process(&mut self, gains_db: [f32; N], rate: SampleRate, samples: &mut [f32]) {
        let highest = rate.0 as f32 * HIGHEST_FRACTION_OF_RATE;
        let settings = self.bands.iter_mut().zip(self.centers).zip(gains_db);
        // A flat band leaves the sound as it is, so it is not worth filtering.
        for ((band, hz), gain_db) in settings.filter(|(_, gain_db)| *gain_db != 0.0) {
            band.set(Coefficients::peaking(
                rate,
                hz.min(highest),
                self.q,
                gain_db,
            ));
            band.process(samples);
        }
    }
}

impl Default for GraphicEqualizer10 {
    fn default() -> Self {
        Self::spaced(GRAPHIC_BANDS_HZ, 1.0)
    }
}

impl Default for GraphicEqualizer20 {
    fn default() -> Self {
        Self::spaced(GRAPHIC_BANDS_20_HZ, 0.5)
    }
}

impl Default for GraphicEqualizer30 {
    fn default() -> Self {
        Self::spaced(GRAPHIC_BANDS_30_HZ, 1.0 / 3.0)
    }
}
