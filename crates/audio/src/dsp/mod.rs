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
mod phaser;
mod pitch;
mod reverb;
mod shape;
mod taps;

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
pub use phaser::{Phaser, PhaserSettings, MAX_PHASER_STAGES};
pub use pitch::PitchShifter;
pub use reverb::Reverb;
pub use shape::{distort, widen};
pub use taps::{AnalogDelay, AnalogDelaySettings, MultitapDelay, Tap};

fn db_to_amplitude(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}
