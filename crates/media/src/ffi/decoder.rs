use std::ptr::{self, NonNull};

use ffmpeg_sys_next as sys;

use super::error::check;
use super::picture::download;
use super::{hardware, Frame, InputContext, Packet, Picture};
use crate::MediaError;

pub(crate) enum Received {
    Frame,
    NeedsInput,
    Drained,
}

pub(crate) struct Decoder {
    context: NonNull<sys::AVCodecContext>,
    hardware: Option<Hardware>,
    /// The format of the last frame received, when it differs from the context's.
    picture_format: Option<sys::AVPixelFormat>,
    #[cfg(windows)]
    gpu: KeptOnGpu,
}

/// Pictures left where D3D12 decoded them, for the GPU to draw from directly.
#[cfg(windows)]
#[derive(Default)]
struct KeptOnGpu {
    wanted: bool,
    holds: super::d3d12::Holds,
    /// The hold of the last picture received, when it was kept.
    last: Option<super::Hold>,
}

/// Decoding on a GPU: frames arrive in `staging` and are copied to system memory.
struct Hardware {
    format: sys::AVPixelFormat,
    staging: Frame,
}

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
        let mut decoder = Self {
            context,
            hardware: None,
            picture_format: None,
            #[cfg(windows)]
            gpu: KeptOnGpu::default(),
        };
        // SAFETY: `codecpar` is valid while `input` is borrowed.
        let is_video = unsafe { (*codecpar).codec_type } == sys::AVMediaType::AVMEDIA_TYPE_VIDEO;
        let device = if is_video {
            hardware::device_for(codec)
        } else {
            None
        };
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
            if let Some((device, format)) = device {
                decoder.hardware = Some(Hardware {
                    format,
                    staging: Frame::new()?,
                });
                raw.hw_device_ctx = device.into_raw();
                raw.opaque = hardware::format_tag(format);
                raw.get_format = Some(hardware::choose_format);
            }
            check(sys::avcodec_open2(raw, codec, ptr::null_mut()))?;
        }
        Ok(decoder)
    }

    /// Sends `None` to signal end of stream so the decoder drains its buffered frames.
    pub(crate) fn send_packet(&mut self, packet: Option<&Packet>) -> Result<(), MediaError> {
        let raw = packet.map_or(ptr::null(), Packet::as_ptr);
        // SAFETY: the context is open, and `raw` is null or a valid packet the caller borrows.
        let code = unsafe { sys::avcodec_send_packet(self.context.as_ptr(), raw) };
        // A corrupt packet is dropped rather than failing the stream; the decoder resyncs
        // at the next keyframe. EOF means the decoder is already draining.
        if code == sys::AVERROR_EOF || code == sys::AVERROR_INVALIDDATA {
            return Ok(());
        }
        check(code).map(drop)
    }

    /// Leaves pictures decoded with D3D12 on the GPU, while not too many are held there.
    #[cfg(windows)]
    pub(crate) fn set_keep_on_gpu(&mut self, wanted: bool) {
        self.gpu.wanted = wanted;
    }

    pub(crate) fn receive_frame(&mut self, frame: &mut Frame) -> Result<Received, MediaError> {
        #[cfg(windows)]
        {
            self.gpu.last = None;
        }
        let Some(hardware) = &mut self.hardware else {
            return receive(self.context, frame);
        };
        let received = receive(self.context, &mut hardware.staging)?;
        if !matches!(received, Received::Frame) {
            return Ok(received);
        }
        let staging = &mut hardware.staging;
        frame.unref();
        if staging.raw().format != hardware.format as i32 {
            // The device declined this stream, so FFmpeg decoded it in software.
            // SAFETY: both frames are valid; `frame` was just unreferenced.
            unsafe { sys::av_frame_move_ref(frame.as_mut_ptr(), staging.as_mut_ptr()) };
            self.picture_format = None;
            return Ok(Received::Frame);
        }
        #[cfg(windows)]
        if self.gpu.wanted && hardware.format == sys::AVPixelFormat::AV_PIX_FMT_D3D12 {
            if let Some(hold) = self.gpu.holds.take() {
                let software = super::picture::format_on_gpu(staging)?;
                // SAFETY: both frames are valid; `frame` was just unreferenced.
                unsafe { sys::av_frame_move_ref(frame.as_mut_ptr(), staging.as_mut_ptr()) };
                self.gpu.last = Some(hold);
                self.picture_format = Some(software);
                return Ok(Received::Frame);
            }
        }
        let copied = download(frame, staging);
        staging.unref();
        self.picture_format = Some(copied?);
        Ok(Received::Frame)
    }

    /// The pixel format of the last frame received.
    /// Moves the picture just received into `frame` out, as this decoder's format.
    pub(crate) fn take_picture(&mut self, frame: &mut Frame) -> Result<Picture, MediaError> {
        let picture = Picture::take(frame, self.picture_format())?;
        #[cfg(windows)]
        let picture = picture.with_hold(self.gpu.last.take());
        Ok(picture)
    }

    fn picture_format(&self) -> sys::AVPixelFormat {
        self.picture_format.unwrap_or(self.raw().pix_fmt)
    }

    /// Discards buffered frames and resets the end-of-stream state, as needed after a seek.
    pub(crate) fn flush(&mut self) {
        // SAFETY: the context is open.
        unsafe { sys::avcodec_flush_buffers(self.context.as_ptr()) };
        if let Some(hardware) = &mut self.hardware {
            hardware.staging.unref();
        }
    }

    pub(super) fn raw(&self) -> &sys::AVCodecContext {
        // SAFETY: the context is valid for the lifetime of `self`.
        unsafe { self.context.as_ref() }
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        let mut raw = self.context.as_ptr();
        // SAFETY: the context came from avcodec_alloc_context3 and is freed exactly once here.
        unsafe { sys::avcodec_free_context(&mut raw) };
    }
}

fn receive(
    context: NonNull<sys::AVCodecContext>,
    frame: &mut Frame,
) -> Result<Received, MediaError> {
    // Frame-threaded and hardware decoders report a corrupt packet here rather than when
    // it is sent. It is skipped the same way, and the frames queued behind it are still
    // asked for, which matters most while draining, when no more packets will follow.
    for _ in 0..MAX_CORRUPT_IN_A_ROW {
        // SAFETY: the context is open and `frame` is a valid, exclusively borrowed frame.
        let code = unsafe { sys::avcodec_receive_frame(context.as_ptr(), frame.as_mut_ptr()) };
        return match code {
            sys::AVERROR_EOF => Ok(Received::Drained),
            code if code == sys::AVERROR(sys::EAGAIN) => Ok(Received::NeedsInput),
            sys::AVERROR_INVALIDDATA => continue,
            code => check(code).map(|_| Received::Frame),
        };
    }
    Ok(Received::NeedsInput)
}

/// More corrupt packets than a frame-threaded decoder can hold at once.
const MAX_CORRUPT_IN_A_ROW: usize = 256;
