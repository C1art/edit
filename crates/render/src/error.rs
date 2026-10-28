use std::fmt;

#[derive(Debug)]
pub enum RenderError {
    NoAdapter,
    FrameSize {
        width: u32,
        height: u32,
        bytes: usize,
    },
    FrameTooLarge {
        width: u32,
        height: u32,
        max: u32,
    },
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoAdapter => f.write_str("no compatible GPU adapter found"),
            Self::FrameSize {
                width,
                height,
                bytes,
            } => write!(f, "{bytes} bytes is not a {width}x{height} RGBA frame"),
            Self::FrameTooLarge { width, height, max } => write!(
                f,
                "{width}x{height} frame exceeds the GPU's {max} pixel texture limit"
            ),
        }
    }
}

impl std::error::Error for RenderError {}
