mod error;
mod output;
mod remix;
mod resampler;

pub use error::AudioError;
pub use output::{AudioOutput, AudioWriter, OutputFormat};
pub use remix::remix_stereo;
pub use resampler::Resampler;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SampleRate(pub u32);
