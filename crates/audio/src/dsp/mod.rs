mod biquad;
mod compressor;
mod delay;

pub use biquad::{Biquad, Coefficients};
pub use compressor::{Compressor, CompressorSettings};
pub use delay::Delay;

fn db_to_amplitude(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}
