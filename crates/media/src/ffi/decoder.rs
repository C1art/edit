use std::ptr::{self, NonNull};

use ffmpeg_sys_next as sys;

use super::error::check;
use super::{Frame, InputContext, Packet};
use crate::MediaError;

pub(crate) enum Received {
    Frame,
    NeedsInput,
    Drained,
}

pub(crate) struct Decoder(NonNull<sys::AVCodecContext>);

// SAFETY: the codec context is exclusively owned and only used through `&mut self`.
unsafe impl Send for Decoder {}

impl Decoder {
    pub(crate) fn open(input: &InputContext, stream_index: usize) -> Result<Self, MediaError> {
        let stream = input.stream(stream_index).ok_or(MediaError::NoDecoder)?;
        // SAFETY: `stream` is a valid stream of the open input, and its `codecpar` is non-null.
        let (codecpar, time_base) = unsafe { ((*stream).codecpar, (*stream).time_base) };
        // SAFETY: `codecpar` is valid for the duration of this call.
        let codec = unsafe { sys::avcodec_find_decoder((*codecpar).codec_id) };
        if codec.is_null() {
            return Err(MediaError::NoDecoder);
        }
        // SAFETY: `codec` is a valid decoder; a null return means allocation failed.
        let context = NonNull::new(unsafe { sys::avcodec_alloc_context3(codec) })
            .ok_or(MediaError::OutOfMemory)?;
        let decoder = Self(context);
        // SAFETY: the context was just allocated and is exclusively owned by `decoder`;
        // `codecpar` stays valid while `input` is borrowed.
        unsafe {
            check(sys::avcodec_parameters_to_context(
                context.as_ptr(),
                codecpar,
            ))?;
            let raw = &mut *context.as_ptr();
            raw.pkt_timebase = time_base;
            // Zero lets FFmpeg pick one thread per core for frame/slice threading.
            raw.thread_count = 0;
            check(sys::avcodec_open2(raw, codec, ptr::null_mut()))?;
        }
        Ok(decoder)
    }

    /// Sends `None` to signal end of stream so the decoder drains its buffered frames.
    pub(crate) fn send_packet(&mut self, packet: Option<&Packet>) -> Result<(), MediaError> {
        let raw = packet.map_or(ptr::null(), Packet::as_ptr);
        // SAFETY: the context is open, and `raw` is null or a valid packet the caller borrows.
        let code = unsafe { sys::avcodec_send_packet(self.0.as_ptr(), raw) };
        // A corrupt packet is dropped rather than failing the stream; the decoder resyncs
        // at the next keyframe. EOF means the decoder is already draining.
        if code == sys::AVERROR_EOF || code == sys::AVERROR_INVALIDDATA {
            return Ok(());
        }
        check(code).map(drop)
    }

    pub(crate) fn receive_frame(&mut self, frame: &mut Frame) -> Result<Received, MediaError> {
        // SAFETY: the context is open and `frame` is a valid, exclusively borrowed frame.
        let code = unsafe { sys::avcodec_receive_frame(self.0.as_ptr(), frame.as_mut_ptr()) };
        match code {
            sys::AVERROR_EOF => Ok(Received::Drained),
            code if code == sys::AVERROR(sys::EAGAIN) => Ok(Received::NeedsInput),
            code => check(code).map(|_| Received::Frame),
        }
    }

    /// Discards buffered frames and resets the end-of-stream state, as needed after a seek.
    pub(crate) fn flush(&mut self) {
        // SAFETY: the context is open.
        unsafe { sys::avcodec_flush_buffers(self.0.as_ptr()) };
    }

    pub(super) fn raw(&self) -> &sys::AVCodecContext {
        // SAFETY: the context is valid for the lifetime of `self`.
        unsafe { self.0.as_ref() }
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        let mut raw = self.0.as_ptr();
        // SAFETY: the context came from avcodec_alloc_context3 and is freed exactly once here.
        unsafe { sys::avcodec_free_context(&mut raw) };
    }
}
