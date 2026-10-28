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
}

impl fmt::Display for MediaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPath(path) => {
                write!(f, "path cannot be passed to FFmpeg: {}", path.display())
            }
            Self::Ffmpeg { code, message } => write!(f, "FFmpeg error {code}: {message}"),
        }
    }
}

impl std::error::Error for MediaError {}
