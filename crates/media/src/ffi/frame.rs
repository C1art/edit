use std::ptr::NonNull;

use ffmpeg_sys_next as sys;

use crate::MediaError;

pub(crate) struct Frame(NonNull<sys::AVFrame>);

// SAFETY: the frame is exclusively owned and FFmpeg frames carry no thread affinity.
unsafe impl Send for Frame {}

impl Frame {
    pub(crate) fn new() -> Result<Self, MediaError> {
        // SAFETY: av_frame_alloc has no preconditions; a null return means allocation failed.
        NonNull::new(unsafe { sys::av_frame_alloc() })
            .map(Self)
            .ok_or(MediaError::OutOfMemory)
    }

    pub(super) fn raw(&self) -> &sys::AVFrame {
        // SAFETY: the frame is valid for the lifetime of `self` and only mutated through `&mut self`.
        unsafe { self.0.as_ref() }
    }

    pub(super) fn as_mut_ptr(&mut self) -> *mut sys::AVFrame {
        self.0.as_ptr()
    }

    /// Presentation time in the stream timebase, falling back to FFmpeg's best guess.
    pub(crate) fn timestamp(&self) -> Option<i64> {
        let frame = self.raw();
        [frame.pts, frame.best_effort_timestamp]
            .into_iter()
            .find(|&ts| ts != sys::AV_NOPTS_VALUE)
    }

    pub(crate) fn unref(&mut self) {
        // SAFETY: the frame is valid; unref releases its buffers and resets its fields.
        unsafe { sys::av_frame_unref(self.0.as_ptr()) };
    }
}

impl Drop for Frame {
    fn drop(&mut self) {
        let mut raw = self.0.as_ptr();
        // SAFETY: the frame came from av_frame_alloc and is freed exactly once here.
        unsafe { sys::av_frame_free(&mut raw) };
    }
}
