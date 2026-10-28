use std::ffi::{c_char, CStr, CString};
use std::path::Path;
use std::ptr::{self, NonNull};

use ffmpeg_sys_next as sys;
use timeline::{Ticks, Timebase};

use super::error::check;
use super::Packet;
use crate::{MediaError, StreamInfo, StreamKind};

/// An opened demuxer whose stream info has been read.
pub(crate) struct InputContext(NonNull<sys::AVFormatContext>);

// SAFETY: the demuxer is exclusively owned and only mutated through `&mut self`.
unsafe impl Send for InputContext {}

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

    pub(super) fn stream(&self, index: usize) -> Option<*mut sys::AVStream> {
        self.raw_streams().get(index).copied()
    }

    fn raw_streams(&self) -> &[*mut sys::AVStream] {
        // SAFETY: the context is open, so `streams` points to `nb_streams` valid,
        // non-null stream pointers that live as long as `self`.
        unsafe {
            let context = self.0.as_ref();
            if context.streams.is_null() {
                return &[];
            }
            std::slice::from_raw_parts(context.streams, context.nb_streams as usize)
        }
    }

    pub(crate) fn best_stream(&self, kind: StreamKind) -> Option<usize> {
        let media_type = match kind {
            StreamKind::Video => sys::AVMediaType::AVMEDIA_TYPE_VIDEO,
            StreamKind::Audio => sys::AVMediaType::AVMEDIA_TYPE_AUDIO,
            StreamKind::Subtitle => sys::AVMediaType::AVMEDIA_TYPE_SUBTITLE,
            StreamKind::Other => return None,
        };
        // SAFETY: the context is open; passing null for `decoder_ret` is allowed.
        let index = unsafe {
            sys::av_find_best_stream(self.0.as_ptr(), media_type, -1, -1, ptr::null_mut(), 0)
        };
        usize::try_from(index).ok()
    }

    pub(crate) fn stream_timebase(&self, index: usize) -> Option<Timebase> {
        // SAFETY: `stream` returns valid stream pointers owned by the open context.
        let time_base = unsafe { (*self.stream(index)?).time_base };
        rational_timebase(time_base)
    }

    /// The stream's average frame rate as `(num, den)`, when the container declares one.
    pub(crate) fn frame_rate(&self, index: usize) -> Option<(u32, u32)> {
        // SAFETY: `stream` returns valid stream pointers owned by the open context.
        let rate = unsafe { (*self.stream(index)?).avg_frame_rate };
        Some((u32::try_from(rate.num).ok()?, u32::try_from(rate.den).ok()?))
    }

    /// The coded picture size declared in the stream header.
    pub(crate) fn frame_size(&self, index: usize) -> Option<(u32, u32)> {
        // SAFETY: `stream` returns valid stream pointers owned by the open context, and
        // every stream has non-null `codecpar`.
        let codecpar = unsafe { &*(*self.stream(index)?).codecpar };
        let width = u32::try_from(codecpar.width).ok().filter(|&w| w > 0)?;
        let height = u32::try_from(codecpar.height).ok().filter(|&h| h > 0)?;
        Some((width, height))
    }

    /// The container's descriptive name, as "QuickTime / MOV".
    pub(crate) fn container(&self) -> String {
        // SAFETY: the context is open, so `iformat` points to a static demuxer whose
        // names are static strings or null.
        let names = unsafe {
            let format = &*self.0.as_ref().iformat;
            text(format.long_name).or_else(|| text(format.name))
        };
        names.unwrap_or_default()
    }

    /// Bits per second over the whole file, when the container knows it.
    pub(crate) fn bit_rate(&self) -> Option<u64> {
        // SAFETY: the context is open.
        let rate = unsafe { self.0.as_ref() }.bit_rate;
        u64::try_from(rate).ok().filter(|&rate| rate > 0)
    }

    /// In `AV_TIME_BASE` units.
    pub(crate) fn start_time(&self) -> Option<i64> {
        // SAFETY: the context is open.
        let start = unsafe { self.0.as_ref() }.start_time;
        (start != sys::AV_NOPTS_VALUE).then_some(start)
    }

    /// In `AV_TIME_BASE` units.
    pub(crate) fn duration(&self) -> Option<i64> {
        // SAFETY: the context is open.
        let duration = unsafe { self.0.as_ref() }.duration;
        (duration != sys::AV_NOPTS_VALUE && duration > 0).then_some(duration)
    }

    /// Returns `false` at end of file.
    pub(crate) fn read_packet(&mut self, packet: &mut Packet) -> Result<bool, MediaError> {
        packet.unref();
        // SAFETY: the context is open and `packet` is a valid, blank packet.
        let code = unsafe { sys::av_read_frame(self.0.as_ptr(), packet.as_mut_ptr()) };
        if code == sys::AVERROR_EOF {
            return Ok(false);
        }
        check(code).map(|_| true)
    }

    /// Seeks to the last keyframe at or before `timestamp`, given in `AV_TIME_BASE` units.
    pub(crate) fn seek(&mut self, timestamp: i64) -> Result<(), MediaError> {
        // SAFETY: the context is open; stream index -1 selects AV_TIME_BASE units.
        check(unsafe {
            sys::avformat_seek_file(self.0.as_ptr(), -1, i64::MIN, timestamp, timestamp, 0)
        })
        .map(drop)
    }

    pub(crate) fn streams(&self) -> Vec<StreamInfo> {
        self.raw_streams()
            .iter()
            .enumerate()
            .filter_map(|(index, &stream)| {
                // SAFETY: each entry is a valid stream with non-null `codecpar`, owned
                // by the context and not mutated while `&self` is held.
                let (stream, codecpar) = unsafe { (&*stream, &*(*stream).codecpar) };
                let timebase = rational_timebase(stream.time_base)?;
                let kind = stream_kind(codecpar.codec_type);
                let audio = kind == StreamKind::Audio;
                // SAFETY: avcodec_get_name returns a static string for any id, never null.
                let codec = unsafe { text(sys::avcodec_get_name(codecpar.codec_id)) };
                Some(StreamInfo {
                    index,
                    kind,
                    timebase,
                    duration: (stream.duration != sys::AV_NOPTS_VALUE)
                        .then_some(Ticks(stream.duration)),
                    codec: codec.unwrap_or_default(),
                    bit_rate: u64::try_from(codecpar.bit_rate)
                        .ok()
                        .filter(|&rate| rate > 0),
                    sample_rate: u32::try_from(codecpar.sample_rate)
                        .ok()
                        .filter(|&rate| audio && rate > 0),
                    channels: u32::try_from(codecpar.ch_layout.nb_channels)
                        .ok()
                        .filter(|&channels| audio && channels > 0),
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

/// # Safety
/// `raw` is null or a NUL-terminated string that outlives the call.
unsafe fn text(raw: *const c_char) -> Option<String> {
    if raw.is_null() {
        return None;
    }
    // SAFETY: `raw` is not null, and the caller promises it is a NUL-terminated string
    // that outlives the call.
    let text = unsafe { CStr::from_ptr(raw) };
    Some(text.to_string_lossy().into_owned())
}

fn rational_timebase(rational: sys::AVRational) -> Option<Timebase> {
    Timebase::new(
        u32::try_from(rational.num).ok()?,
        u32::try_from(rational.den).ok()?,
    )
}

fn stream_kind(media_type: sys::AVMediaType) -> StreamKind {
    match media_type {
        sys::AVMediaType::AVMEDIA_TYPE_VIDEO => StreamKind::Video,
        sys::AVMediaType::AVMEDIA_TYPE_AUDIO => StreamKind::Audio,
        sys::AVMediaType::AVMEDIA_TYPE_SUBTITLE => StreamKind::Subtitle,
        _ => StreamKind::Other,
    }
}
