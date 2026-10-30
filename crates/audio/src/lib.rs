mod dsp;
mod error;
mod output;
mod remix;
mod resampler;

pub use dsp::{
    distort, synthesize_impulse, widen, AnalogDelay, AnalogDelaySettings, Biquad, ClickRemover,
    Coefficients, Compressor, CompressorSettings, Convolver, DeEsser, DeHum, DeNoise, DeReverb,
    Delay, Expander, FilterBand, FilterDesign, Gate, GraphicEqualizer, GraphicEqualizer10,
    GraphicEqualizer20, GraphicEqualizer30, ImpulseShape, LevelerSettings, Limiter, LoudnessMeter,
    ModulatedDelay, MultibandCompressor, MultibandSettings, MultitapDelay, MusicFeatures, Phaser,
    PhaserSettings, PitchShifter, Reverb, ScientificFilter, ScientificSettings, SpeechLeveler,
    Sweep, Tap, GRAPHIC_BANDS_20_HZ, GRAPHIC_BANDS_30_HZ, GRAPHIC_BANDS_HZ, MAX_HARMONICS,
    MAX_PHASER_STAGES, RETIME_HOP,
};
pub use error::AudioError;
pub use output::{output_devices, AudioOutput, AudioWriter, OutputDevice, OutputFormat};
pub use remix::remix_stereo;
pub use resampler::Resampler;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SampleRate(pub u32);
