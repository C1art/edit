use std::ptr;

use ffmpeg_sys_next as sys;

use super::error::check;
use super::Frame;
use crate::MediaError;

/// A decoded picture kept as the decoder left it, so that pictures a seek skips over
/// are never converted.
pub(crate) struct Picture {
    frame: Frame,
    format: sys::AVPixelFormat,
    /// Counts against the decoder's pictures on the GPU while this one is.
    #[cfg(windows)]
    _hold: Option<super::Hold>,
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
            #[cfg(windows)]
            _hold: None,
        })
    }

    #[cfg(windows)]
    pub(super) fn with_hold(self, hold: Option<super::Hold>) -> Self {
        Self {
            _hold: hold,
            ..self
        }
    }

    pub(crate) fn frame(&self) -> &Frame {
        &self.frame
    }

    /// The format of the picture's pixels; for one on the GPU, the format they have
    /// there.
    pub(crate) fn format(&self) -> sys::AVPixelFormat {
        self.format
    }

    /// Whether the picture is still in the decoding device's memory.
    pub(crate) fn on_gpu(&self) -> bool {
        !self.frame.raw().hw_frames_ctx.is_null()
    }

    /// A copy in system memory of a picture on the GPU.
    pub(crate) fn to_memory(&self) -> Result<Self, MediaError> {
        let mut frame = Frame::new()?;
        let format = download(&mut frame, &self.frame)?;
        Self::take(&mut frame, format)
    }

    /// The picture in system memory, read back if it is on the GPU.
    pub(crate) fn into_memory(self) -> Result<Self, MediaError> {
        if self.on_gpu() {
            self.to_memory()
        } else {
            Ok(self)
        }
    }
}

/// Reads a picture on the GPU back into the empty `target`, returning the format it
/// has there.
pub(super) fn download(
    target: &mut Frame,
    source: &Frame,
) -> Result<sys::AVPixelFormat, MediaError> {
    let software = format_on_gpu(source)?;
    target.raw_mut().format = software as i32;
    // SAFETY: `target` is empty with its format set, so FFmpeg allocates it to match the
    // frame it copies from, which is only read.
    unsafe {
        check(sys::av_hwframe_transfer_data(
            target.as_mut_ptr(),
            ptr::from_ref(source.raw()),
            0,
        ))?;
        check(sys::av_frame_copy_props(
            target.as_mut_ptr(),
            ptr::from_ref(source.raw()),
        ))?;
    }
    Ok(software)
}

/// The format of the pixels of a frame on the GPU, as they are there.
pub(super) fn format_on_gpu(frame: &Frame) -> Result<sys::AVPixelFormat, MediaError> {
    let frames = frame.raw().hw_frames_ctx;
    if frames.is_null() {
        return Err(MediaError::InvalidFrame);
    }
    // SAFETY: a frame on the GPU holds its frames context, whose data is an
    // AVHWFramesContext.
    Ok(unsafe { (*(*frames).data.cast::<sys::AVHWFramesContext>()).sw_format })
}
