use std::ffi::CString;
use std::path::Path;
use std::ptr::{self, NonNull};

use ffmpeg_sys_next as sys;
use timeline::{Chapter, Timebase};

use super::error::check;
use super::{Encoder, Packet, PacketTimebase};
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

    /// Adds chapters for players to skip between, before the header is written.
    pub(crate) fn set_chapters(
        &mut self,
        chapters: &[Chapter],
        timebase: Timebase,
    ) -> Result<(), MediaError> {
        if chapters.is_empty() {
            return Ok(());
        }
        let too_many = || MediaError::InvalidSettings("too many chapters");
        let time_base = sys::AVRational {
            num: i32::try_from(timebase.num()).map_err(|_| too_many())?,
            den: i32::try_from(timebase.den()).map_err(|_| too_many())?,
        };
        let titles = chapters
            .iter()
            .map(|chapter| CString::new(chapter.title.replace('\0', "")))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| too_many())?;
        let context = self.0.as_ptr();
        // SAFETY: the context is valid, its header is not written and it has no chapters
        // yet. The array and each chapter come from FFmpeg's allocator and are handed to
        // the context as soon as they exist, so avformat_free_context frees them however
        // far this gets; `nb_chapters` only counts chapters already in the array.
        unsafe {
            debug_assert!((*context).chapters.is_null());
            let array = sys::av_calloc(chapters.len(), size_of::<*mut sys::AVChapter>())
                .cast::<*mut sys::AVChapter>();
            if array.is_null() {
                return Err(MediaError::OutOfMemory);
            }
            (*context).chapters = array;
            for (index, (chapter, title)) in chapters.iter().zip(&titles).enumerate() {
                let raw = sys::av_mallocz(size_of::<sys::AVChapter>()).cast::<sys::AVChapter>();
                if raw.is_null() {
                    return Err(MediaError::OutOfMemory);
                }
                *array.add(index) = raw;
                (*context).nb_chapters += 1;
                let raw = &mut *raw;
                raw.id = i64::try_from(index).map_err(|_| too_many())?;
                raw.time_base = time_base;
                raw.start = chapter.range.start.0;
                raw.end = chapter.range.end.0;
                check(sys::av_dict_set(
                    &mut raw.metadata,
                    c"title".as_ptr(),
                    title.as_ptr(),
                    0,
                ))?;
            }
        }
        Ok(())
    }

    pub(crate) fn write_header(&mut self) -> Result<(), MediaError> {
        // SAFETY: the context is valid and all streams have been added.
        check(unsafe { sys::avformat_write_header(self.0.as_ptr(), ptr::null_mut()) }).map(drop)
    }

    /// Hands the packet's data to the muxer, rescaling its timestamps from `encoder`'s
    /// timebase to the stream's. The packet is left blank.
    pub(crate) fn write_packet(
        &mut self,
        packet: &mut Packet,
        stream_index: usize,
        timebase: PacketTimebase,
    ) -> Result<(), MediaError> {
        let timebase = timebase.0;
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
