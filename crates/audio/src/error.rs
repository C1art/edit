use std::fmt;

#[derive(Debug)]
pub enum AudioError {
    NoOutputDevice,
    Device(cpal::Error),
    /// The device's default stream format isn't `f32`, the only format the mixer produces.
    UnsupportedSampleFormat,
    InvalidFormat,
    ResamplerSetup(rubato::ResamplerConstructionError),
    Resample(rubato::ResampleError),
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoOutputDevice => f.write_str("no audio output device available"),
            Self::Device(error) => write!(f, "audio device error: {error}"),
            Self::UnsupportedSampleFormat => {
                f.write_str("audio device does not accept 32-bit float samples")
            }
            Self::InvalidFormat => f.write_str("sample rate and channel count must be non-zero"),
            Self::ResamplerSetup(error) => write!(f, "cannot create resampler: {error}"),
            Self::Resample(error) => write!(f, "resampling failed: {error}"),
        }
    }
}

impl std::error::Error for AudioError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Device(error) => Some(error),
            Self::ResamplerSetup(error) => Some(error),
            Self::Resample(error) => Some(error),
            Self::NoOutputDevice | Self::UnsupportedSampleFormat | Self::InvalidFormat => None,
        }
    }
}
