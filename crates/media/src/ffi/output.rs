use std::ffi::CString;
use std::path::Path;
use std::ptr::{self, NonNull};

use ffmpeg_sys_next as sys;

use super::error::check;
use super::{Encoder, Packet};
use crate::MediaError;

/// A muxer writing one file, with the container picked from the file extension.
pub(crate) struct OutputContext(NonNull<sys::AVFormatContext>);

// SAFETY: the muxer is exclusively owned and only mutated through `&mut self`.
unsafe impl Send for OutputContext {}

impl OutputContext {
    pub(crate) fn create(path: &Path) -> Result<Self, MediaError> {
        let unsupported = || MediaError::UnsupportedPath(path.to_owned());
        let c_path =
            CString::new(path.to_str().ok_or_else(unsupported)?).map_err(|_| unsupported())?;
        let mut raw = ptr::null_mut();
        // SAFETY: `raw` starts null so FFmpeg allocates the context, and `c_path` is a
        // NUL-terminated string that outlives the call.
        check(unsafe {
            sys::avformat_alloc_output_context2(&mut raw, ptr::null(), ptr::null(), c_path.as_ptr())
        })?;
        let output = Self(NonNull::new(raw).ok_or(MediaError::OutOfMemory)?);
        if output.flags() & sys::AVFMT_NOFILE == 0 {
            // SAFETY: the context is exclusively owned and its `pb` is still null; Drop
            // closes it again.
            check(unsafe {
                sys::avio_open(&mut (*raw).pb, c_path.as_ptr(), sys::AVIO_FLAG_WRITE)
            })?;
        }
        Ok(output)
    }

    fn flags(&self) -> i32 {
        // SAFETY: an allocated output context always has a non-null `oformat`.
        unsafe { (*self.0.as_ref().oformat).flags }
    }

    /// Whether encoders must put codec headers in extradata instead of in-band.
    pub(crate) fn needs_global_header(&self) -> bool {
        self.flags() & sys::AVFMT_GLOBALHEADER != 0
    }

    /// Adds a stream carrying `encoder`'s output and returns its index.
    pub(crate) fn add_stream(&mut self, encoder: &Encoder) -> Result<usize, MediaError> {
        // SAFETY: the context is valid and the header is not written yet; a null codec
        // is allowed because the parameters are copied from the encoder below.
        let stream =
            NonNull::new(unsafe { sys::avformat_new_stream(self.0.as_ptr(), ptr::null()) })
                .ok_or(MediaError::OutOfMemory)?;
        // SAFETY: the stream belongs to this context and has non-null `codecpar`; the
        // encoder is open.
        unsafe {
            let stream = &mut *stream.as_ptr();
            check(sys::avcodec_parameters_from_context(
                stream.codecpar,
                encoder.raw(),
            ))?;
            stream.time_base = encoder.raw().time_base;
            usize::try_from(stream.index).map_err(|_| MediaError::OutOfMemory)
        }
    }

    pub(crate) fn write_header(&mut self) -> Result<(), MediaError> {
        // SAFETY: the context is valid and all streams have been added.
        check(unsafe { sys::avformat_write_header(self.0.as_ptr(), ptr::null_mut()) }).map(drop)
    }

    /// Hands the packet's data to the muxer, rescaling its timestamps from `timebase`
    /// to the stream's. The packet is left blank.
    pub(crate) fn write_packet(
        &mut self,
        packet: &mut Packet,
        stream_index: usize,
        timebase: sys::AVRational,
    ) -> Result<(), MediaError> {
        let stream = self.stream_timebase(stream_index)?;
        let index = i32::try_from(stream_index).map_err(|_| MediaError::InvalidFrame)?;
        let raw = packet.as_mut_ptr();
        // SAFETY: `raw` is a valid packet holding encoder output, and the header has been
        // written; av_interleaved_write_frame takes ownership of its reference.
        check(unsafe {
            (*raw).stream_index = index;
            sys::av_packet_rescale_ts(raw, timebase, stream);
            sys::av_interleaved_write_frame(self.0.as_ptr(), raw)
        })
        .map(drop)
    }

    pub(crate) fn write_trailer(&mut self) -> Result<(), MediaError> {
        // SAFETY: the header was written and every packet has been handed over.
        check(unsafe { sys::av_write_trailer(self.0.as_ptr()) }).map(drop)
    }

    fn stream_timebase(&self, index: usize) -> Result<sys::AVRational, MediaError> {
        // SAFETY: `streams` holds `nb_streams` valid stream pointers owned by the context.
        unsafe {
            let context = self.0.as_ref();
            if index >= context.nb_streams as usize {
                return Err(MediaError::InvalidFrame);
            }
            Ok((**context.streams.add(index)).time_base)
        }
    }
}

impl Drop for OutputContext {
    fn drop(&mut self) {
        let no_file = self.flags() & sys::AVFMT_NOFILE != 0;
        // SAFETY: the file was opened in `create` unless the format has none, and the
        // context came from avformat_alloc_output_context2; both are released once here.
        unsafe {
            if !no_file {
                sys::avio_closep(&mut (*self.0.as_ptr()).pb);
            }
            sys::avformat_free_context(self.0.as_ptr());
        }
    }
}
