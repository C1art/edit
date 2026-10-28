use std::ptr::NonNull;

use ffmpeg_sys_next as sys;

use crate::MediaError;

pub(crate) struct Packet(NonNull<sys::AVPacket>);

// SAFETY: the packet is exclusively owned and FFmpeg packets carry no thread affinity.
unsafe impl Send for Packet {}

impl Packet {
    pub(crate) fn new() -> Result<Self, MediaError> {
        // SAFETY: av_packet_alloc has no preconditions; a null return means allocation failed.
        NonNull::new(unsafe { sys::av_packet_alloc() })
            .map(Self)
            .ok_or(MediaError::OutOfMemory)
    }

    pub(super) fn as_ptr(&self) -> *const sys::AVPacket {
        self.0.as_ptr()
    }

    pub(super) fn as_mut_ptr(&mut self) -> *mut sys::AVPacket {
        self.0.as_ptr()
    }

    pub(crate) fn stream_index(&self) -> Option<usize> {
        // SAFETY: the packet is valid for the lifetime of `self`.
        usize::try_from(unsafe { self.0.as_ref() }.stream_index).ok()
    }

    /// Sets the duration, in the packet's timebase, unless the encoder already did.
    pub(crate) fn fill_duration(&mut self, duration: i64) {
        // SAFETY: the packet is valid and exclusively borrowed.
        let raw = unsafe { self.0.as_mut() };
        if raw.duration <= 0 {
            raw.duration = duration;
        }
    }

    pub(crate) fn unref(&mut self) {
        // SAFETY: the packet is valid; unref resets it to the blank state and frees its buffer.
        unsafe { sys::av_packet_unref(self.0.as_ptr()) };
    }
}

impl Drop for Packet {
    fn drop(&mut self) {
        let mut raw = self.0.as_ptr();
        // SAFETY: the packet came from av_packet_alloc and is freed exactly once here.
        unsafe { sys::av_packet_free(&mut raw) };
    }
}
