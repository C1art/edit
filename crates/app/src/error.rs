use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum AppError {
    Io(std::io::Error),
    CreateFolder {
        path: PathBuf,
        error: std::io::Error,
    },
    Project(project::ProjectError),
    Timeline(timeline::TimelineError),
    Media(media::MediaError),
    Playback(playback::PlaybackError),
    Cancelled,
    /// A worker thread went away without saying how its work ended.
    WorkerStopped,
    /// The named source changed since it was queued and no longer renders on its own.
    NotRenderable(String),
    NoRenderer,
    UnnamedWorkspace,
    WorkspaceTaken(String),
    /// The settings file there couldn't be read, so it's left as it was.
    SettingHeld,
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::CreateFolder { path, error } => {
                write!(f, "cannot create {}: {error}", path.display())
            }
            Self::Project(error) => write!(f, "{error}"),
            Self::Timeline(error) => write!(f, "{error}"),
            Self::Media(error) => write!(f, "{error}"),
            Self::Playback(error) => write!(f, "{error}"),
            Self::Cancelled => f.write_str("cancelled"),
            Self::WorkerStopped => f.write_str("the work stopped unexpectedly"),
            Self::NotRenderable(name) => write!(f, "{name} can no longer be rendered"),
            Self::NoRenderer => f.write_str("there is no GPU renderer"),
            Self::UnnamedWorkspace => f.write_str("Name the workspace"),
            Self::WorkspaceTaken(name) => write!(f, "There is already a workspace called {name}"),
            Self::SettingHeld => f.write_str(
                "the file there couldn't be read when the editor started, so it is left unchanged",
            ),
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) | Self::CreateFolder { error, .. } => Some(error),
            Self::Project(error) => Some(error),
            Self::Timeline(error) => Some(error),
            Self::Media(error) => Some(error),
            Self::Playback(error) => Some(error),
            Self::Cancelled
            | Self::WorkerStopped
            | Self::NotRenderable(_)
            | Self::NoRenderer
            | Self::UnnamedWorkspace
            | Self::WorkspaceTaken(_)
            | Self::SettingHeld => None,
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<project::ProjectError> for AppError {
    fn from(error: project::ProjectError) -> Self {
        Self::Project(error)
    }
}

impl From<timeline::TimelineError> for AppError {
    fn from(error: timeline::TimelineError) -> Self {
        Self::Timeline(error)
    }
}

impl From<media::MediaError> for AppError {
    fn from(error: media::MediaError) -> Self {
        Self::Media(error)
    }
}

impl From<playback::PlaybackError> for AppError {
    fn from(error: playback::PlaybackError) -> Self {
        Self::Playback(error)
    }
}
