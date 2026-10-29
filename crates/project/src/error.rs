use std::fmt;

#[derive(Debug)]
pub enum ProjectError {
    UnsupportedVersion(u32),
    MissingVersion,
    Syntax {
        line: usize,
        message: String,
    },
    /// The document parsed but doesn't describe a valid project.
    Invalid(String),
    Io(std::io::Error),
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion(version) => write!(
                f,
                "project format version {version} is newer than this build supports"
            ),
            Self::MissingVersion => f.write_str("not a project file: it has no format version"),
            Self::Syntax { line, message } => {
                write!(f, "damaged project file, line {line}: {message}")
            }
            Self::Invalid(message) => write!(f, "damaged project file: {message}"),
            Self::Io(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ProjectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ProjectError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl serde::ser::Error for ProjectError {
    fn custom<T: fmt::Display>(message: T) -> Self {
        Self::Invalid(message.to_string())
    }
}

impl serde::de::Error for ProjectError {
    fn custom<T: fmt::Display>(message: T) -> Self {
        Self::Invalid(message.to_string())
    }
}
