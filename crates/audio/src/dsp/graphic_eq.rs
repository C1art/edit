use super::{Biquad, Coefficients};
use crate::SampleRate;

/// The centres of the graphic equalizer's bands, an octave apart.
pub const GRAPHIC_BANDS_HZ: [f32; 10] = [
    31.0, 63.0, 125.0, 250.0, 500.0, 1_000.0, 2_000.0, 4_000.0, 8_000.0, 16_000.0,
];
/// Each bell spans an octave, meeting its neighbours where they have fallen halfway.
const OCTAVE_Q: f32 = std::f32::consts::SQRT_2;

/// A bell at each of [`GRAPHIC_BANDS_HZ`], each boosting or cutting by its own gain.
#[derive(Debug, Clone, Copy, Default)]
pub struct GraphicEqualizer {
    bands: [Biquad; 10],
}

impl GraphicEqualizer {
    pub fn reset(&mut self) {
        self.bands.iter_mut().for_each(Biquad::reset);
    }

    pub fn process(&mut self, gains_db: [f32; 10], rate: SampleRate, samples: &mut [f32]) {
        let settings = self.bands.iter_mut().zip(GRAPHIC_BANDS_HZ).zip(gains_db);
        // A flat band leaves the sound as it is, so it is not worth filtering.
        for ((band, hz), gain_db) in settings.filter(|(_, gain_db)| *gain_db != 0.0) {
            band.set(Coefficients::peaking(rate, hz, OCTAVE_Q, gain_db));
            band.process(samples);
        }
    }
}
