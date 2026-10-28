use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaError {
    /// FFmpeg on Windows takes UTF-8 paths without interior NULs.
    UnsupportedPath(PathBuf),
    Ffmpeg {
        code: i32,
        message: String,
    },
    NoPlayableStream,
    NoDecoder,
    InvalidFrame,
    OutOfMemory,
}

impl fmt::Display for MediaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPath(path) => {
                write!(f, "path cannot be passed to FFmpeg: {}", path.display())
            }
            Self::Ffmpeg { code, message } => write!(f, "FFmpeg error {code}: {message}"),
            Self::NoPlayableStream => f.write_str("file has no video or audio stream"),
            Self::NoDecoder => f.write_str("no decoder available for the stream's codec"),
            Self::InvalidFrame => f.write_str("decoder produced a frame that cannot be converted"),
            Self::OutOfMemory => f.write_str("FFmpeg could not allocate memory"),
        }
    }
}

impl std::error::Error for MediaError {}
