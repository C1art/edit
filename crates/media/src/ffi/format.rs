use std::ffi::CString;
use std::path::Path;
use std::ptr::{self, NonNull};

use ffmpeg_sys_next as sys;
use timeline::{Ticks, Timebase};

use super::error::check;
use crate::{MediaError, StreamInfo, StreamKind};

/// An opened demuxer whose stream info has been read.
pub(crate) struct InputContext(NonNull<sys::AVFormatContext>);

impl InputContext {
    pub(crate) fn open(path: &Path) -> Result<Self, MediaError> {
        let unsupported = || MediaError::UnsupportedPath(path.to_owned());
        let c_path =
            CString::new(path.to_str().ok_or_else(unsupported)?).map_err(|_| unsupported())?;

        let mut raw = ptr::null_mut();
        // SAFETY: `raw` starts null so FFmpeg allocates the context, and `c_path` is a
        // NUL-terminated string that outlives the call. On failure FFmpeg frees the
        // context and leaves `raw` null.
        check(unsafe {
            sys::avformat_open_input(&mut raw, c_path.as_ptr(), ptr::null(), ptr::null_mut())
        })?;
        let input = Self(NonNull::new(raw).ok_or_else(unsupported)?);

        // SAFETY: the context was opened above and is exclusively owned by `input`.
        check(unsafe { sys::avformat_find_stream_info(input.0.as_ptr(), ptr::null_mut()) })?;
        Ok(input)
    }

    pub(crate) fn streams(&self) -> Vec<StreamInfo> {
        // SAFETY: the context is open, so `streams` points to `nb_streams` valid,
        // non-null stream pointers that live as long as `self`.
        let streams = unsafe {
            let context = self.0.as_ref();
            if context.streams.is_null() {
                return Vec::new();
            }
            std::slice::from_raw_parts(context.streams, context.nb_streams as usize)
        };
        streams
            .iter()
            .enumerate()
            .filter_map(|(index, &stream)| {
                // SAFETY: each entry is a valid stream with non-null `codecpar`, owned
                // by the context and not mutated while `&self` is held.
                let (stream, codecpar) = unsafe { (&*stream, &*(*stream).codecpar) };
                let timebase = Timebase::new(
                    u32::try_from(stream.time_base.num).ok()?,
                    u32::try_from(stream.time_base.den).ok()?,
                )?;
                Some(StreamInfo {
                    index,
                    kind: stream_kind(codecpar.codec_type),
                    timebase,
                    duration: (stream.duration != sys::AV_NOPTS_VALUE)
                        .then_some(Ticks(stream.duration)),
                })
            })
            .collect()
    }
}

impl Drop for InputContext {
    fn drop(&mut self) {
        let mut raw = self.0.as_ptr();
        // SAFETY: the context came from avformat_open_input and is closed exactly once here.
        unsafe { sys::avformat_close_input(&mut raw) };
    }
}

fn stream_kind(media_type: sys::AVMediaType) -> StreamKind {
    match media_type {
        sys::AVMediaType::AVMEDIA_TYPE_VIDEO => StreamKind::Video,
        sys::AVMediaType::AVMEDIA_TYPE_AUDIO => StreamKind::Audio,
        sys::AVMediaType::AVMEDIA_TYPE_SUBTITLE => StreamKind::Subtitle,
        _ => StreamKind::Other,
    }
}
