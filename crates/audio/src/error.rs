use std::fmt;

#[derive(Debug)]
pub enum AudioError {
    NoOutputDevice,
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoOutputDevice => f.write_str("no audio output device available"),
        }
    }
}

impl std::error::Error for AudioError {}
