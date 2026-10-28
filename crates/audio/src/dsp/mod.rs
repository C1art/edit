mod biquad;
mod compressor;
mod delay;
mod dynamics;
mod loudness;
mod reverb;

pub use biquad::{Biquad, Coefficients};
pub use compressor::{Compressor, CompressorSettings};
pub use delay::Delay;
pub use dynamics::{Gate, Limiter};
pub use loudness::LoudnessMeter;
pub use reverb::Reverb;

fn db_to_amplitude(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}
