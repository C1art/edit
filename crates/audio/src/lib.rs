mod dsp;
mod error;
mod output;
mod remix;
mod resampler;

pub use dsp::{
    Biquad, Coefficients, Compressor, CompressorSettings, DeEsser, DeHum, DeNoise, Delay, Gate,
    GraphicEqualizer, Limiter, LoudnessMeter, ModulatedDelay, MultibandCompressor,
    MultibandSettings, PitchShifter, Reverb, Sweep, GRAPHIC_BANDS_HZ, MAX_HARMONICS,
};
pub use error::AudioError;
pub use output::{AudioOutput, AudioWriter, OutputFormat};
pub use remix::remix_stereo;
pub use resampler::Resampler;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SampleRate(pub u32);
