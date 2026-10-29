mod biquad;
mod compressor;
mod deesser;
mod dehum;
mod delay;
mod denoise;
mod dynamics;
mod fft;
mod graphic_eq;
mod loudness;
mod modulation;
mod multiband;
mod pitch;
mod reverb;

pub use biquad::{Biquad, Coefficients};
pub use compressor::{Compressor, CompressorSettings};
pub use deesser::DeEsser;
pub use dehum::{DeHum, MAX_HARMONICS};
pub use delay::Delay;
pub use denoise::DeNoise;
pub use dynamics::{Gate, Limiter};
pub use graphic_eq::{GraphicEqualizer, GRAPHIC_BANDS_HZ};
pub use loudness::LoudnessMeter;
pub use modulation::{ModulatedDelay, Sweep};
pub use multiband::{MultibandCompressor, MultibandSettings};
pub use pitch::PitchShifter;
pub use reverb::Reverb;

fn db_to_amplitude(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}
