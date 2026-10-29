use std::ptr::{self, NonNull};

use ffmpeg_sys_next as sys;
use timeline::FieldOrder;

use super::{Decoder, Frame};
use crate::deinterlace::deinterlace;
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
    /// Interlaced pictures are rebuilt from their first field at the decoded size,
    /// before any resizing mixes the fields' rows.
    fields: FieldOrder,
    /// Resizes deinterlaced RGBA, keyed by its input and output sizes.
    resize: Option<(NonNull<sys::SwsContext>, [i32; 4])>,
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

    pub(crate) fn set_field_order(&mut self, fields: FieldOrder) {
        self.fields = fields;
    }

    pub(crate) fn convert(
        &mut self,
        decoder: &Decoder,
        frame: &Frame,
    ) -> Result<(u32, u32, Vec<u8>), MediaError> {
        let raw = frame.raw();
        let decoded = (raw.width, raw.height);
        let wanted = self.output_size.unwrap_or(decoded);
        if self.fields.is_progressive() {
            return self.convert_to(decoder, frame, wanted);
        }
        let (width, height, mut pixels) = self.convert_to(decoder, frame, decoded)?;
        deinterlace(&mut pixels, width as usize, height as usize, self.fields);
        if wanted == decoded {
            return Ok((width, height, pixels));
        }
        self.resize_rgba(decoded, wanted, &pixels)
    }

    fn convert_to(
        &mut self,
        decoder: &Decoder,
        frame: &Frame,
        (output_width, output_height): (i32, i32),
    ) -> Result<(u32, u32, Vec<u8>), MediaError> {
        let raw = frame.raw();
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

    fn resize_rgba(
        &mut self,
        (width, height): (i32, i32),
        (output_width, output_height): (i32, i32),
        pixels: &[u8],
    ) -> Result<(u32, u32, Vec<u8>), MediaError> {
        let key = [width, height, output_width, output_height];
        let context = match self.resize {
            Some((context, current)) if current == key => context,
            _ => {
                self.free_resize();
                // SAFETY: plain constructor call with sizes from decoded frames and a
                // validated output size; null means the sizes are unsupported.
                let context = NonNull::new(unsafe {
                    sys::sws_getContext(
                        width,
                        height,
                        sys::AVPixelFormat::AV_PIX_FMT_RGBA,
                        output_width,
                        output_height,
                        sys::AVPixelFormat::AV_PIX_FMT_RGBA,
                        sys::SWS_BILINEAR | sys::SWS_ACCURATE_RND,
                        ptr::null_mut(),
                        ptr::null_mut(),
                        ptr::null(),
                    )
                })
                .ok_or(MediaError::InvalidFrame)?;
                self.resize = Some((context, key));
                context
            }
        };
        let out_width = u32::try_from(output_width).map_err(|_| MediaError::InvalidFrame)?;
        let out_height = u32::try_from(output_height).map_err(|_| MediaError::InvalidFrame)?;
        let stride = output_width
            .checked_mul(4)
            .ok_or(MediaError::InvalidFrame)?;
        let source_stride = width.checked_mul(4).ok_or(MediaError::InvalidFrame)?;
        let mut resized = vec![0u8; out_width as usize * out_height as usize * 4];
        let source = [pixels.as_ptr(), ptr::null(), ptr::null(), ptr::null()];
        let destination = [
            resized.as_mut_ptr(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
        ];
        // SAFETY: `pixels` holds `height` rows of `source_stride` bytes, as converted at
        // the decoded size, and `resized` holds `output_height` rows of `stride` bytes;
        // both are single RGBA planes matching the context's formats and sizes.
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
        Ok((out_width, out_height, resized))
    }

    fn free_resize(&mut self) {
        if let Some((context, _)) = self.resize.take() {
            // SAFETY: the context came from sws_getContext and is freed exactly once here.
            unsafe { sys::sws_freeContext(context.as_ptr()) };
        }
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
        self.free_resize();
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
