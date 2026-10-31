use std::fmt;
use std::io;

#[derive(Debug)]
pub enum SpeechError {
    WorkerSpawn(io::Error),
    /// The model file couldn't be loaded or run.
    Model(String),
    Download(String),
}

impl fmt::Display for SpeechError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WorkerSpawn(error) => write!(f, "could not start speech recognition: {error}"),
            Self::Model(message) => write!(f, "speech recognition failed: {message}"),
            Self::Download(message) => write!(f, "could not download the model: {message}"),
        }
    }
}

impl std::error::Error for SpeechError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::WorkerSpawn(error) => Some(error),
            Self::Model(_) | Self::Download(_) => None,
        }
    }
}

impl From<whisper_rs::WhisperError> for SpeechError {
    fn from(error: whisper_rs::WhisperError) -> Self {
        Self::Model(error.to_string())
    }
}
