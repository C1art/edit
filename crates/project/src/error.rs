use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectError {
    UnsupportedVersion(u32),
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion(version) => write!(
                f,
                "project format version {version} is newer than this build supports"
            ),
        }
    }
}

impl std::error::Error for ProjectError {}
