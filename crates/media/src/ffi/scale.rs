use std::ptr::{self, NonNull};

use ffmpeg_sys_next as sys;

use super::{Decoder, Frame};
use crate::MediaError;

#[derive(Clone, Copy, PartialEq, Eq)]
struct ScalerKey {
    width: i32,
    height: i32,
    format: sys::AVPixelFormat,
    colorspace: sys::AVColorSpace,
    range: sys::AVColorRange,
    output_width: i32,
    output_height: i32,
}

/// Converts decoded frames to tightly packed, gamma-encoded RGBA8, rebuilding the
/// swscale context whenever the source geometry or color description changes.
#[derive(Default)]
pub(crate) struct Scaler {
    context: Option<(NonNull<sys::SwsContext>, ScalerKey)>,
    /// Pictures are resized to this; `None` keeps the decoded size.
    output_size: Option<(i32, i32)>,
}

// SAFETY: the swscale context is exclusively owned and only used through `&mut self`.
unsafe impl Send for Scaler {}

impl Scaler {
    pub(crate) fn set_output_size(&mut self, size: Option<(u32, u32)>) -> Result<(), MediaError> {
        let invalid = || MediaError::InvalidSettings("picture size out of range");
        self.output_size = size
            .map(|(width, height)| {
                let width = i32::try_from(width).map_err(|_| invalid())?;
                let height = i32::try_from(height).map_err(|_| invalid())?;
                if width == 0 || height == 0 {
                    return Err(invalid());
                }
                Ok((width, height))
            })
            .transpose()?;
        Ok(())
    }

    pub(crate) fn convert(
        &mut self,
        decoder: &Decoder,
        frame: &Frame,
    ) -> Result<(u32, u32, Vec<u8>), MediaError> {
        let raw = frame.raw();
        let (output_width, output_height) = self.output_size.unwrap_or((raw.width, raw.height));
        let key = ScalerKey {
            width: raw.width,
            height: raw.height,
            format: decoder.picture_format(),
            colorspace: raw.colorspace,
            range: raw.color_range,
            output_width,
            output_height,
        };
        let context = self.context_for(key)?;

        let width = u32::try_from(output_width).map_err(|_| MediaError::InvalidFrame)?;
        let height = u32::try_from(output_height).map_err(|_| MediaError::InvalidFrame)?;
        let stride = output_width
            .checked_mul(4)
            .ok_or(MediaError::InvalidFrame)?;
        let mut pixels = vec![0u8; width as usize * height as usize * 4];
        let destination = [
            pixels.as_mut_ptr(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
        ];
        let destination_stride = [stride, 0, 0, 0];
        // SAFETY: the source planes and strides come from a decoded frame matching `key`, and
        // `pixels` holds exactly `output_height` rows of `stride` bytes for the single RGBA
        // plane.
        let rows = unsafe {
            sys::sws_scale(
                context.as_ptr(),
                raw.data.as_ptr().cast(),
                raw.linesize.as_ptr(),
                0,
                key.height,
                destination.as_ptr(),
                destination_stride.as_ptr(),
            )
        };
        if rows != output_height {
            return Err(MediaError::InvalidFrame);
        }
        Ok((width, height, pixels))
    }

    fn context_for(&mut self, key: ScalerKey) -> Result<NonNull<sys::SwsContext>, MediaError> {
        if let Some((context, current)) = self.context {
            if current == key {
                return Ok(context);
            }
        }
        self.free();
        // SAFETY: plain constructor call with validated sizes; null means unsupported input.
        let context = NonNull::new(unsafe {
            sys::sws_getContext(
                key.width,
                key.height,
                key.format,
                key.output_width,
                key.output_height,
                sys::AVPixelFormat::AV_PIX_FMT_RGBA,
                sys::SWS_BILINEAR | sys::SWS_ACCURATE_RND | sys::SWS_FULL_CHR_H_INT,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null(),
            )
        })
        .ok_or(MediaError::InvalidFrame)?;
        // SAFETY: `context` was just created; sws_getCoefficients returns static tables.
        // A failure here only means the format has no YUV matrix (e.g. RGB input).
        unsafe {
            sys::sws_setColorspaceDetails(
                context.as_ptr(),
                sys::sws_getCoefficients(matrix_for(key)),
                i32::from(key.range == sys::AVColorRange::AVCOL_RANGE_JPEG),
                sys::sws_getCoefficients(sys::SWS_CS_DEFAULT),
                1,
                0,
                1 << 16,
                1 << 16,
            );
        }
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

impl Drop for Scaler {
    fn drop(&mut self) {
        self.free();
    }
}

/// Untagged streams follow the common convention: HD and larger is BT.709, SD is BT.601.
fn matrix_for(key: ScalerKey) -> i32 {
    use sys::AVColorSpace::*;
    match key.colorspace {
        AVCOL_SPC_BT709 => sys::SWS_CS_ITU709,
        AVCOL_SPC_BT2020_NCL | AVCOL_SPC_BT2020_CL => sys::SWS_CS_BT2020,
        AVCOL_SPC_SMPTE170M | AVCOL_SPC_BT470BG => sys::SWS_CS_ITU601,
        AVCOL_SPC_SMPTE240M => sys::SWS_CS_SMPTE240M,
        AVCOL_SPC_FCC => sys::SWS_CS_FCC,
        _ if key.height >= 720 => sys::SWS_CS_ITU709,
        _ => sys::SWS_CS_ITU601,
    }
}
