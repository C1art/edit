use ffmpeg_sys_next as sys;

use super::Frame;
use crate::MediaError;

/// A decoded picture kept as the decoder left it, so that pictures a seek skips over
/// are never converted.
pub(crate) struct Picture {
    frame: Frame,
    format: sys::AVPixelFormat,
}

// SAFETY: the frame is owned and only read through `&self`; its buffers are
// reference-counted with atomic counts.
unsafe impl Sync for Picture {}

impl Picture {
    /// Takes the picture out of `frame`, leaving it empty for the next one.
    pub(crate) fn take(frame: &mut Frame, format: sys::AVPixelFormat) -> Result<Self, MediaError> {
        let mut taken = Frame::new()?;
        // SAFETY: both frames are valid and `taken` is empty.
        unsafe { sys::av_frame_move_ref(taken.as_mut_ptr(), frame.as_mut_ptr()) };
        Ok(Self {
            frame: taken,
            format,
        })
    }

    pub(crate) fn frame(&self) -> &Frame {
        &self.frame
    }

    pub(crate) fn format(&self) -> sys::AVPixelFormat {
        self.format
    }
}
