use std::fmt;

#[derive(Debug)]
pub enum PlaybackError {
    Media(media::MediaError),
    Render(render::RenderError),
    Audio(audio::AudioError),
    WorkerSpawn(std::io::Error),
    /// An export's decoding thread ended before its last frame.
    DecodeStopped,
    /// A finished export could not be moved over the file at its path.
    Replace {
        path: std::path::PathBuf,
        error: std::io::Error,
    },
    Lut {
        path: std::path::PathBuf,
        reason: String,
    },
    /// An Image Matte Key's picture could not be read.
    ImageMatte {
        path: std::path::PathBuf,
        error: media::MediaError,
    },
}

impl fmt::Display for PlaybackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Media(error) => write!(f, "media: {error}"),
            Self::Render(error) => write!(f, "render: {error}"),
            Self::Audio(error) => write!(f, "audio: {error}"),
            Self::WorkerSpawn(error) => write!(f, "cannot start decode thread: {error}"),
            Self::DecodeStopped => write!(f, "decoding stopped before the last frame"),
            Self::Replace { path, error } => {
                write!(f, "cannot replace {}: {error}", path.display())
            }
            Self::Lut { path, reason } => {
                write!(f, "cannot use the LUT {}: {reason}", path.display())
            }
            Self::ImageMatte { path, error } => {
                write!(f, "cannot use the matte {}: {error}", path.display())
            }
        }
    }
}

impl std::error::Error for PlaybackError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Media(error) => Some(error),
            Self::Render(error) => Some(error),
            Self::Audio(error) => Some(error),
            Self::WorkerSpawn(error) => Some(error),
            Self::Replace { error, .. } => Some(error),
            Self::ImageMatte { error, .. } => Some(error),
            Self::Lut { .. } | Self::DecodeStopped => None,
        }
    }
}

impl From<media::MediaError> for PlaybackError {
    fn from(error: media::MediaError) -> Self {
        Self::Media(error)
    }
}

impl From<render::RenderError> for PlaybackError {
    fn from(error: render::RenderError) -> Self {
        Self::Render(error)
    }
}

impl From<audio::AudioError> for PlaybackError {
    fn from(error: audio::AudioError) -> Self {
        Self::Audio(error)
    }
}
