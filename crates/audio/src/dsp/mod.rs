mod biquad;
mod compressor;
mod deesser;
mod delay;
mod dynamics;
mod loudness;
mod modulation;
mod pitch;
mod reverb;

pub use biquad::{Biquad, Coefficients};
pub use compressor::{Compressor, CompressorSettings};
pub use deesser::DeEsser;
pub use delay::Delay;
pub use dynamics::{Gate, Limiter};
pub use loudness::LoudnessMeter;
pub use modulation::{ModulatedDelay, Sweep};
pub use pitch::PitchShifter;
pub use reverb::Reverb;

fn db_to_amplitude(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}
