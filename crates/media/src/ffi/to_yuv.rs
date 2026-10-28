use std::ptr::{self, NonNull};

use ffmpeg_sys_next as sys;

use super::encoder::VIDEO_FORMAT;
use super::Frame;
use crate::MediaError;

/// Converts sRGB-encoded RGBA8 to limited-range BT.709 4:2:0 for the encoder.
pub(crate) struct RgbaToYuv {
    context: NonNull<sys::SwsContext>,
    width: i32,
    height: i32,
}

// SAFETY: the swscale context is exclusively owned and only used through `&mut self`.
unsafe impl Send for RgbaToYuv {}

impl RgbaToYuv {
    pub(crate) fn new((width, height): (u32, u32)) -> Result<Self, MediaError> {
        let invalid = || MediaError::InvalidSettings("frame size out of range");
        let width = i32::try_from(width).map_err(|_| invalid())?;
        let height = i32::try_from(height).map_err(|_| invalid())?;
        // SAFETY: plain constructor call with validated sizes; null means unsupported input.
        let context = NonNull::new(unsafe {
            sys::sws_getContext(
                width,
                height,
                sys::AVPixelFormat::AV_PIX_FMT_RGBA,
                width,
                height,
                VIDEO_FORMAT,
                sys::SWS_BILINEAR | sys::SWS_ACCURATE_RND | sys::SWS_FULL_CHR_H_INP,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null(),
            )
        })
        .ok_or_else(invalid)?;
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
        })
    }

    pub(crate) fn convert(&mut self, rgba: &[u8], frame: &mut Frame) -> Result<(), MediaError> {
        let expected = self.width as usize * self.height as usize * 4;
        if rgba.len() != expected {
            return Err(MediaError::InvalidFrame);
        }
        frame.make_writable()?;
        let raw = frame.raw_mut();
        if (raw.width, raw.height) != (self.width, self.height) {
            return Err(MediaError::InvalidFrame);
        }
        let source = [rgba.as_ptr(), ptr::null(), ptr::null(), ptr::null()];
        let source_stride = [self.width * 4, 0, 0, 0];
        // SAFETY: `rgba` holds `height` rows of `width * 4` bytes, and the frame is a
        // writable YUV 4:2:0 picture of the same size whose planes match its linesizes.
        let rows = unsafe {
            sys::sws_scale(
                self.context.as_ptr(),
                source.as_ptr(),
                source_stride.as_ptr(),
                0,
                self.height,
                raw.data.as_ptr(),
                raw.linesize.as_ptr(),
            )
        };
        if rows != self.height {
            return Err(MediaError::InvalidFrame);
        }
        Ok(())
    }
}

impl Drop for RgbaToYuv {
    fn drop(&mut self) {
        // SAFETY: the context came from sws_getContext and is freed exactly once here.
        unsafe { sys::sws_freeContext(self.context.as_ptr()) };
    }
}
