use std::mem::MaybeUninit;
use std::ptr::{self, NonNull};

use ffmpeg_sys_next as sys;

use super::error::check;
use super::{Decoder, Frame};
use crate::MediaError;

pub(crate) const OUTPUT_CHANNELS: u16 = 2;

#[derive(Clone, Copy, PartialEq, Eq)]
struct ConverterKey {
    format: sys::AVSampleFormat,
    sample_rate: i32,
    channels: i32,
    order: sys::AVChannelOrder,
    mask: u64,
}

/// Converts decoded audio of any layout and sample format to interleaved stereo `f32` at
/// the source rate. Rate conversion is left to the audio crate's resampler.
#[derive(Default)]
pub(crate) struct SampleConverter {
    context: Option<(NonNull<sys::SwrContext>, ConverterKey)>,
}

// SAFETY: the swresample context is exclusively owned and only used through `&mut self`.
unsafe impl Send for SampleConverter {}

impl SampleConverter {
    /// Returns the sample rate and the interleaved stereo samples.
    pub(crate) fn convert(
        &mut self,
        decoder: &Decoder,
        frame: &Frame,
    ) -> Result<(u32, Vec<f32>), MediaError> {
        let raw = frame.raw();
        let layout = &raw.ch_layout;
        let key = ConverterKey {
            format: decoder.raw().sample_fmt,
            sample_rate: raw.sample_rate,
            channels: layout.nb_channels,
            order: layout.order,
            // SAFETY: `mask` is the active union member for native-order layouts, and reading
            // it for other orders only feeds the cache key.
            mask: unsafe { layout.u.mask },
        };
        let context = self.context_for(key, layout)?;
        let sample_rate = u32::try_from(key.sample_rate).map_err(|_| MediaError::InvalidFrame)?;

        // SAFETY: the context is initialized; this only queries its buffered sample count.
        let capacity =
            check(unsafe { sys::swr_get_out_samples(context.as_ptr(), raw.nb_samples) })?;
        let capacity_frames = usize::try_from(capacity).map_err(|_| MediaError::InvalidFrame)?;
        let mut samples = vec![0f32; capacity_frames * usize::from(OUTPUT_CHANNELS)];
        let mut output = samples.as_mut_ptr().cast::<u8>();
        // SAFETY: `output` points to room for `capacity` interleaved stereo f32 frames, and
        // `extended_data` holds the frame's `nb_samples` input samples in the context's format.
        let converted = check(unsafe {
            sys::swr_convert(
                context.as_ptr(),
                &mut output,
                capacity,
                raw.extended_data as *mut *const u8,
                raw.nb_samples,
            )
        })?;
        samples.truncate(usize::try_from(converted).unwrap_or(0) * usize::from(OUTPUT_CHANNELS));
        Ok((sample_rate, samples))
    }

    fn context_for(
        &mut self,
        key: ConverterKey,
        input_layout: &sys::AVChannelLayout,
    ) -> Result<NonNull<sys::SwrContext>, MediaError> {
        if let Some((context, current)) = self.context {
            if current == key {
                return Ok(context);
            }
        }
        self.free();

        let mut output_layout = MaybeUninit::<sys::AVChannelLayout>::zeroed();
        // SAFETY: av_channel_layout_default fully initializes the layout; a native-order
        // layout owns no heap memory, but it is uninitialized below all the same.
        let mut output_layout = unsafe {
            sys::av_channel_layout_default(output_layout.as_mut_ptr(), i32::from(OUTPUT_CHANNELS));
            output_layout.assume_init()
        };
        let mut raw = ptr::null_mut();
        // SAFETY: both layouts are valid for the call, and `raw` starts null so FFmpeg
        // allocates the context.
        let allocated = check(unsafe {
            sys::swr_alloc_set_opts2(
                &mut raw,
                &output_layout,
                sys::AVSampleFormat::AV_SAMPLE_FMT_FLT,
                key.sample_rate,
                input_layout,
                key.format,
                key.sample_rate,
                0,
                ptr::null_mut(),
            )
        });
        // SAFETY: the layout was initialized above and is not used afterwards.
        unsafe { sys::av_channel_layout_uninit(&mut output_layout) };
        allocated?;
        let context = NonNull::new(raw).ok_or(MediaError::OutOfMemory)?;
        self.context = Some((context, key));
        // SAFETY: the context was configured by swr_alloc_set_opts2 and is owned by `self`.
        if let Err(error) = check(unsafe { sys::swr_init(context.as_ptr()) }) {
            self.free();
            return Err(error);
        }
        Ok(context)
    }

    fn free(&mut self) {
        if let Some((context, _)) = self.context.take() {
            let mut raw = context.as_ptr();
            // SAFETY: the context came from swr_alloc_set_opts2 and is freed exactly once here.
            unsafe { sys::swr_free(&mut raw) };
        }
    }
}

impl Drop for SampleConverter {
    fn drop(&mut self) {
        self.free();
    }
}
