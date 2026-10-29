use std::ptr::{self, NonNull};

use ffmpeg_sys_next as sys;

use super::sws_flags::{SWS_ACCURATE_RND, SWS_BILINEAR, SWS_FULL_CHR_H_INT};
use timeline::FieldOrder;

use super::error::check;
use super::{Frame, Picture, RgbaResizer};
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
    resize: RgbaResizer,
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

    pub(crate) fn convert(&mut self, picture: &Picture) -> Result<(u32, u32, Vec<u8>), MediaError> {
        let (frame, format) = (picture.frame(), picture.format());
        let raw = frame.raw();
        let decoded = (raw.width, raw.height);
        let wanted = self.output_size.unwrap_or(decoded);
        if self.fields.is_progressive() {
            return self.convert_to(format, frame, wanted);
        }
        let (width, height, mut pixels) = self.convert_to(format, frame, decoded)?;
        deinterlace(&mut pixels, width as usize, height as usize, self.fields);
        if wanted == decoded {
            return Ok((width, height, pixels));
        }
        let [width, height, output_width, output_height] =
            [decoded.0, decoded.1, wanted.0, wanted.1].map(i32::unsigned_abs);
        let resized =
            self.resize
                .resize((width, height), (output_width, output_height), &pixels)?;
        Ok((output_width, output_height, resized))
    }

    fn convert_to(
        &mut self,
        format: sys::AVPixelFormat,
        frame: &Frame,
        (output_width, output_height): (i32, i32),
    ) -> Result<(u32, u32, Vec<u8>), MediaError> {
        let raw = frame.raw();
        let key = ScalerKey {
            width: raw.width,
            height: raw.height,
            format,
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
        let mut target = Frame::new()?;
        // SAFETY: `target` borrows `pixels` through a buffer whose release does nothing,
        // and is dropped before `pixels` is returned; swscale drops its own reference
        // before sws_scale_frame returns. `pixels` holds exactly `output_height` rows of
        // `stride` bytes for the single RGBA plane, and `frame` is a decoded frame
        // matching `key`.
        unsafe {
            let buffer = sys::av_buffer_create(
                pixels.as_mut_ptr(),
                pixels.len(),
                Some(borrowed_buffer_release),
                ptr::null_mut(),
                0,
            );
            if buffer.is_null() {
                return Err(MediaError::OutOfMemory);
            }
            let raw = target.raw_mut();
            raw.buf[0] = buffer;
            raw.data[0] = pixels.as_mut_ptr();
            raw.linesize[0] = stride;
            raw.width = output_width;
            raw.height = output_height;
            raw.format = sys::AVPixelFormat::AV_PIX_FMT_RGBA as i32;
            check(sys::sws_scale_frame(
                context.as_ptr(),
                target.as_mut_ptr(),
                ptr::from_ref(frame.raw()),
            ))?;
        }
        drop(target);
        Ok((width, height, pixels))
    }

    fn context_for(&mut self, key: ScalerKey) -> Result<NonNull<sys::SwsContext>, MediaError> {
        if let Some((context, current)) = self.context {
            if current == key {
                return Ok(context);
            }
        }
        self.free();
        let context = new_context(key)?;
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
            // SAFETY: the context came from sws_alloc_context and is freed exactly once here.
            unsafe { sys::sws_freeContext(context.as_ptr()) };
        }
    }
}

impl Drop for Scaler {
    fn drop(&mut self) {
        self.free();
    }
}

/// A context converting `key`'s pictures to RGBA on as many threads as there are
/// cores, as a 4K picture takes one core longer than a frame lasts.
fn new_context(key: ScalerKey) -> Result<NonNull<sys::SwsContext>, MediaError> {
    // SAFETY: plain allocator call; null means out of memory.
    let context =
        NonNull::new(unsafe { sys::sws_alloc_context() }).ok_or(MediaError::OutOfMemory)?;
    let options = [
        (c"srcw", i64::from(key.width)),
        (c"srch", i64::from(key.height)),
        (c"src_format", key.format as i64),
        (c"dstw", i64::from(key.output_width)),
        (c"dsth", i64::from(key.output_height)),
        (c"dst_format", sys::AVPixelFormat::AV_PIX_FMT_RGBA as i64),
        (
            c"sws_flags",
            i64::from(SWS_BILINEAR | SWS_ACCURATE_RND | SWS_FULL_CHR_H_INT),
        ),
        (c"threads", 0),
    ];
    // SAFETY: `context` is a fresh, uninitialized context and every name is one of its
    // options; init validates the sizes and formats, failing on unsupported input.
    let initialized = unsafe {
        options
            .iter()
            .try_for_each(|(name, value)| {
                check(sys::av_opt_set_int(
                    context.as_ptr().cast(),
                    name.as_ptr(),
                    *value,
                    0,
                ))
                .map(drop)
            })
            .and_then(|()| {
                check(sys::sws_init_context(
                    context.as_ptr(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                ))
            })
    };
    if let Err(error) = initialized {
        // SAFETY: the context is ours and freed exactly once here.
        unsafe { sys::sws_freeContext(context.as_ptr()) };
        return Err(error);
    }
    Ok(context)
}

/// Pixels handed to swscale stay owned by their `Vec`.
unsafe extern "C" fn borrowed_buffer_release(_opaque: *mut std::ffi::c_void, _data: *mut u8) {}

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
