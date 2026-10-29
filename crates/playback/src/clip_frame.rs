use std::sync::Arc;

use media::{MediaError, PlanarFrame, VideoFrame};
use timeline::Ticks;

/// A decoded picture, as RGBA or still in the decoder's planes.
#[derive(Debug, Clone)]
pub(crate) enum ClipFrame {
    Rgba(Arc<VideoFrame>),
    Planar(Arc<PlanarFrame>),
}

impl ClipFrame {
    pub(crate) fn pts(&self) -> Ticks {
        match self {
            Self::Rgba(frame) => frame.pts,
            Self::Planar(frame) => frame.pts,
        }
    }

    pub(crate) fn size(&self) -> (u32, u32) {
        match self {
            Self::Rgba(frame) => (frame.width, frame.height),
            Self::Planar(frame) => (frame.width, frame.height),
        }
    }

    pub(crate) fn is(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Rgba(this), Self::Rgba(other)) => Arc::ptr_eq(this, other),
            (Self::Planar(this), Self::Planar(other)) => Arc::ptr_eq(this, other),
            _ => false,
        }
    }

    /// The picture as RGBA, converted on this thread when it is planar.
    pub(crate) fn rgba(&self) -> Result<Arc<VideoFrame>, MediaError> {
        match self {
            Self::Rgba(frame) => Ok(Arc::clone(frame)),
            Self::Planar(frame) => frame.to_rgba().map(Arc::new),
        }
    }
}
