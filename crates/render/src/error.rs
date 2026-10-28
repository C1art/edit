use std::fmt;

#[derive(Debug)]
pub enum RenderError {
    NoAdapter,
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoAdapter => f.write_str("no compatible GPU adapter found"),
        }
    }
}

impl std::error::Error for RenderError {}
