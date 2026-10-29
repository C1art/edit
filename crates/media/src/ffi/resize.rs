use std::ptr::{self, NonNull};

use ffmpeg_sys_next as sys;

use super::sws_flags::{SWS_ACCURATE_RND, SWS_BILINEAR};

use crate::MediaError;

/// Resizes tightly packed RGBA8 pictures, keeping its swscale context while the sizes
/// stay the same.
#[derive(Default)]
pub struct RgbaResizer {
    context: Option<(NonNull<sys::SwsContext>, [i32; 4])>,
}

// SAFETY: the swscale context is exclusively owned and only used through `&mut self`.
unsafe impl Send for RgbaResizer {}

impl RgbaResizer {
    /// `rgba`, `from.0 * 4` bytes a row, scaled to `to`.
    pub fn resize(
        &mut self,
        from: (u32, u32),
        to: (u32, u32),
        rgba: &[u8],
    ) -> Result<Vec<u8>, MediaError> {
        let side = |length: u32| match i32::try_from(length) {
            Ok(length) if length > 0 => Ok(length),
            _ => Err(MediaError::InvalidFrame),
        };
        let (width, height) = (side(from.0)?, side(from.1)?);
        let (output_width, output_height) = (side(to.0)?, side(to.1)?);
        let source_stride = width.checked_mul(4).ok_or(MediaError::InvalidFrame)?;
        let stride = output_width
            .checked_mul(4)
            .ok_or(MediaError::InvalidFrame)?;
        if rgba.len() != from.0 as usize * from.1 as usize * 4 {
            return Err(MediaError::InvalidFrame);
        }
        let context = self.context_for([width, height, output_width, output_height])?;
        let mut resized = vec![0u8; to.0 as usize * to.1 as usize * 4];
        let source = [rgba.as_ptr(), ptr::null(), ptr::null(), ptr::null()];
        let destination = [
            resized.as_mut_ptr(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
        ];
        // SAFETY: `rgba` holds `height` rows of `source_stride` bytes, as checked above,
        // and `resized` holds `output_height` rows of `stride` bytes; both are single
        // RGBA planes matching the context's formats and sizes.
        let rows = unsafe {
            sys::sws_scale(
                context.as_ptr(),
                source.as_ptr(),
                [source_stride, 0, 0, 0].as_ptr(),
                0,
                height,
                destination.as_ptr(),
                [stride, 0, 0, 0].as_ptr(),
            )
        };
        if rows != output_height {
            return Err(MediaError::InvalidFrame);
        }
        Ok(resized)
    }

    fn context_for(&mut self, key: [i32; 4]) -> Result<NonNull<sys::SwsContext>, MediaError> {
        if let Some((context, current)) = self.context {
            if current == key {
                return Ok(context);
            }
        }
        self.free();
        let [width, height, output_width, output_height] = key;
        // SAFETY: plain constructor call with positive sizes; null means swscale
        // cannot convert between them.
        let context = NonNull::new(unsafe {
            sys::sws_getContext(
                width,
                height,
                sys::AVPixelFormat::AV_PIX_FMT_RGBA,
                output_width,
                output_height,
                sys::AVPixelFormat::AV_PIX_FMT_RGBA,
                SWS_BILINEAR | SWS_ACCURATE_RND,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null(),
            )
        })
        .ok_or(MediaError::InvalidFrame)?;
        self.context = Some((context, key));
        Ok(context)
    }

    fn free(&mut self) {
        if let Some((context, _)) = self.context.take() {
            // SAFETY: the context came from sws_getContext and is freed exactly once here.
            unsafe { sys::sws_freeContext(context.as_ptr()) };
        }
    }
}

impl Drop for RgbaResizer {
    fn drop(&mut self) {
        self.free();
    }
}
