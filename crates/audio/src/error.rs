use std::fmt;

use cpal::ErrorKind;

#[derive(Debug)]
pub enum AudioError {
    NoOutputDevice,
    /// The output device chosen by name isn't connected.
    MissingDevice(String),
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
            Self::MissingDevice(name) => write!(f, "the audio device “{name}” is not connected"),
            Self::Device(error) => match error.kind() {
                // cpal's own words for these speak of a "requested" device even when the
                // system default was the one asked for.
                ErrorKind::DeviceNotAvailable => {
                    f.write_str("the audio device isn't available or was disconnected")
                }
                ErrorKind::DeviceBusy => f.write_str("another program is using the audio device"),
                ErrorKind::HostUnavailable => {
                    f.write_str("the system's audio service isn't running")
                }
                ErrorKind::PermissionDenied => {
                    f.write_str("the system denied access to the audio device")
                }
                _ => write!(f, "{error}"),
            },
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
            Self::NoOutputDevice
            | Self::MissingDevice(_)
            | Self::UnsupportedSampleFormat
            | Self::InvalidFormat => None,
        }
    }
}
