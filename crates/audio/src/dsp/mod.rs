mod biquad;
mod click;
mod compressor;
mod convolution;
mod deesser;
mod dehum;
mod delay;
mod denoise;
mod dereverb;
mod dynamics;
mod expander;
mod fft;
mod graphic_eq;
mod impulse;
mod leveler;
mod loudness;
mod modulation;
mod multiband;
mod phaser;
mod pitch;
mod retime;
mod reverb;
mod scientific;
mod shape;
mod stft;
mod taps;

pub use biquad::{Biquad, Coefficients};
pub use click::ClickRemover;
pub use compressor::{Compressor, CompressorSettings};
pub use convolution::Convolver;
pub use deesser::DeEsser;
pub use dehum::{DeHum, MAX_HARMONICS};
pub use delay::Delay;
pub use denoise::DeNoise;
pub use dereverb::DeReverb;
pub use dynamics::{Gate, Limiter};
pub use expander::Expander;
pub use graphic_eq::{
    GraphicEqualizer, GraphicEqualizer10, GraphicEqualizer20, GraphicEqualizer30,
    GRAPHIC_BANDS_20_HZ, GRAPHIC_BANDS_30_HZ, GRAPHIC_BANDS_HZ,
};
pub use impulse::{synthesize_impulse, ImpulseShape};
pub use leveler::{LevelerSettings, SpeechLeveler};
pub use loudness::LoudnessMeter;
pub use modulation::{ModulatedDelay, Sweep};
pub use multiband::{MultibandCompressor, MultibandSettings};
pub use phaser::{Phaser, PhaserSettings, MAX_PHASER_STAGES};
pub use pitch::PitchShifter;
pub use retime::{MusicFeatures, RETIME_HOP};
pub use reverb::Reverb;
pub use scientific::{FilterBand, FilterDesign, ScientificFilter, ScientificSettings};
pub use shape::{distort, widen};
pub use taps::{AnalogDelay, AnalogDelaySettings, MultitapDelay, Tap};

/// Zero for values far below hearing. Feedback that decays after the sound stops
/// otherwise reaches subnormal floats, which are many times slower to compute with.
fn flush(value: f32) -> f32 {
    if value.abs() < 1e-20 {
        0.0
    } else {
        value
    }
}

fn db_to_amplitude(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}
