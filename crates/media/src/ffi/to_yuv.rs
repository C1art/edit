use std::ptr::{self, NonNull};

use ffmpeg_sys_next as sys;

use super::error::check;
use super::sws_context::{borrowed_buffer_release, threaded_context};
use super::sws_flags::{SWS_BICUBIC, SWS_FULL_CHR_H_INP};
use super::Frame;
use crate::MediaError;

/// Converts sRGB-encoded RGBA8 to limited-range BT.709 in the encoder's pixel format,
/// scaling it to the encoded size, on as many threads as there are cores: on one, with
/// exact rounding, a 1080p picture took longer than an export frame's share of time,
/// holding back the whole export.
pub(crate) struct RgbaToYuv {
    context: NonNull<sys::SwsContext>,
    width: i32,
    height: i32,
    output: (i32, i32),
}

// SAFETY: the swscale context is exclusively owned and only used through `&mut self`.
unsafe impl Send for RgbaToYuv {}

impl RgbaToYuv {
    pub(crate) fn new(
        (width, height): (u32, u32),
        (out_width, out_height): (u32, u32),
        format: sys::AVPixelFormat,
    ) -> Result<Self, MediaError> {
        let invalid = || MediaError::InvalidSettings("frame size out of range");
        let width = i32::try_from(width).map_err(|_| invalid())?;
        let height = i32::try_from(height).map_err(|_| invalid())?;
        let out_width = i32::try_from(out_width).map_err(|_| invalid())?;
        let out_height = i32::try_from(out_height).map_err(|_| invalid())?;
        let options = [
            (c"srcw", i64::from(width)),
            (c"srch", i64::from(height)),
            (c"src_format", sys::AVPixelFormat::AV_PIX_FMT_RGBA as i64),
            (c"dstw", i64::from(out_width)),
            (c"dsth", i64::from(out_height)),
            (c"dst_format", format as i64),
            // Exact rounding would take swscale off its fast paths for at most one level of
            // difference; full chroma input keeps 4:4:4 outputs sharp.
            (c"sws_flags", i64::from(SWS_BICUBIC | SWS_FULL_CHR_H_INP)),
            (c"threads", 0),
        ];
        let context = threaded_context(&options)?;
        // SAFETY: `context` was just created; sws_getCoefficients returns static tables.
        unsafe {
            let bt709 = sys::sws_getCoefficients(sys::SWS_CS_ITU709);
            sys::sws_setColorspaceDetails(
                context.as_ptr(),
                bt709,
                1,
                bt709,
                0,
                0,
                1 << 16,
                1 << 16,
            );
        }
        Ok(Self {
            context,
            width,
            height,
            output: (out_width, out_height),
        })
    }

    pub(crate) fn convert(&mut self, rgba: &[u8], frame: &mut Frame) -> Result<(), MediaError> {
        let expected = self.width as usize * self.height as usize * 4;
        if rgba.len() != expected {
            return Err(MediaError::InvalidFrame);
        }
        frame.make_writable()?;
        if (frame.raw().width, frame.raw().height) != self.output {
            return Err(MediaError::InvalidFrame);
        }
        let mut source = Frame::new()?;
        // SAFETY: `source` borrows `rgba` through a buffer whose release does nothing, and
        // is dropped before this returns; swscale only reads it, and drops its own
        // reference before sws_scale_frame returns. `rgba` holds `height` rows of
        // `width * 4` bytes, and `frame` is a writable picture of the output size and
        // format.
        unsafe {
            let buffer = sys::av_buffer_create(
                rgba.as_ptr().cast_mut(),
                rgba.len(),
                Some(borrowed_buffer_release),
                ptr::null_mut(),
                0,
            );
            if buffer.is_null() {
                return Err(MediaError::OutOfMemory);
            }
            let raw = source.raw_mut();
            raw.buf[0] = buffer;
            raw.data[0] = rgba.as_ptr().cast_mut();
            raw.linesize[0] = self.width * 4;
            raw.width = self.width;
            raw.height = self.height;
            raw.format = sys::AVPixelFormat::AV_PIX_FMT_RGBA as i32;
            check(sys::sws_scale_frame(
                self.context.as_ptr(),
                frame.as_mut_ptr(),
                source.as_mut_ptr(),
            ))?;
        }
        Ok(())
    }
}

impl Drop for RgbaToYuv {
    fn drop(&mut self) {
        // SAFETY: the context came from sws_alloc_context and is freed exactly once here.
        unsafe { sys::sws_freeContext(self.context.as_ptr()) };
    }
}
