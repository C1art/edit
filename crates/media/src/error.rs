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
    /// Not a format FFmpeg reads, or damaged or incomplete.
    UnreadableFile,
    /// The container doesn't say how long the media is, so it can't be edited yet.
    UnknownDuration,
    NoDecoder,
    NoEncoder,
    InvalidSettings(&'static str),
    InvalidFrame,
    OutOfMemory,
    CannotWrite {
        path: PathBuf,
        reason: String,
    },
    /// The caller stopped the work before it finished.
    Cancelled,
}

impl fmt::Display for MediaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPath(path) => {
                write!(f, "path cannot be passed to FFmpeg: {}", path.display())
            }
            Self::Ffmpeg { code, message } => write!(f, "FFmpeg error {code}: {message}"),
            Self::NoPlayableStream => f.write_str("file has no video or audio stream"),
            Self::UnreadableFile => {
                f.write_str("not a media file that can be read, or the file is damaged")
            }
            Self::UnknownDuration => f.write_str("file does not declare its duration"),
            Self::NoDecoder => f.write_str("no decoder available for the stream's codec"),
            Self::NoEncoder => f.write_str("FFmpeg has no encoder for the export format"),
            Self::InvalidSettings(reason) => write!(f, "invalid export settings: {reason}"),
            Self::InvalidFrame => f.write_str("decoder produced a frame that cannot be converted"),
            Self::OutOfMemory => f.write_str("FFmpeg could not allocate memory"),
            Self::CannotWrite { path, reason } => {
                write!(f, "cannot write {}: {reason}", path.display())
            }
            Self::Cancelled => f.write_str("cancelled"),
        }
    }
}

impl std::error::Error for MediaError {}
