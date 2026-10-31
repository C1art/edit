use ffmpeg_sys_next as sys;

use super::error::check;
use super::Frame;
use crate::MediaError;

const STEREO: usize = 2;

impl Frame {
    /// A frame with its own picture buffer.
    pub(crate) fn for_video(
        (width, height): (u32, u32),
        format: sys::AVPixelFormat,
    ) -> Result<Self, MediaError> {
        let mut frame = Self::new()?;
        let invalid = || MediaError::InvalidSettings("frame size out of range");
        let raw = frame.raw_mut();
        raw.format = format as i32;
        raw.width = i32::try_from(width).map_err(|_| invalid())?;
        raw.height = i32::try_from(height).map_err(|_| invalid())?;
        frame.allocate()?;
        Ok(frame)
    }

    /// A stereo frame with room for `capacity` samples per channel.
    pub(crate) fn for_audio(
        sample_rate: u32,
        capacity: usize,
        format: sys::AVSampleFormat,
    ) -> Result<Self, MediaError> {
        let mut frame = Self::new()?;
        let invalid = || MediaError::InvalidSettings("audio frame size out of range");
        let raw = frame.raw_mut();
        raw.format = format as i32;
        raw.sample_rate = i32::try_from(sample_rate).map_err(|_| invalid())?;
        raw.nb_samples = i32::try_from(capacity).map_err(|_| invalid())?;
        // SAFETY: the frame is freshly allocated, so its layout owns no memory yet.
        unsafe { sys::av_channel_layout_default(&mut raw.ch_layout, STEREO as i32) };
        frame.allocate()?;
        Ok(frame)
    }

    fn allocate(&mut self) -> Result<(), MediaError> {
        // SAFETY: format and size fields were set by the caller; alignment 0 lets FFmpeg
        // choose one suitable for SIMD.
        check(unsafe { sys::av_frame_get_buffer(self.as_mut_ptr(), 0) }).map(drop)
    }

    /// The encoder may still hold a reference to the last buffer; this copies it away
    /// first so the frame can be refilled.
    pub(crate) fn make_writable(&mut self) -> Result<(), MediaError> {
        // SAFETY: the frame has buffers from `allocate`.
        check(unsafe { sys::av_frame_make_writable(self.as_mut_ptr()) }).map(drop)
    }

    /// Both in the encoder timebase. Without a duration the muxer can't tell how long
    /// the last frame lasts and cuts the stream short by it.
    pub(crate) fn set_timing(&mut self, pts: i64, duration: i64) {
        let raw = self.raw_mut();
        raw.pts = pts;
        raw.duration = duration;
    }

    /// Fills an audio frame in planar or packed float or packed 16- or 32-bit from
    /// interleaved stereo, which must fit its capacity.
    pub(crate) fn fill_stereo(
        &mut self,
        interleaved: &[f32],
        capacity: usize,
    ) -> Result<(), MediaError> {
        let samples = interleaved.len() / STEREO;
        if samples > capacity {
            return Err(MediaError::InvalidFrame);
        }
        self.make_writable()?;
        let raw = self.raw_mut();
        raw.nb_samples = i32::try_from(samples).map_err(|_| MediaError::InvalidFrame)?;
        let format = raw.format;
        if format == sys::AVSampleFormat::AV_SAMPLE_FMT_FLTP as i32 {
            // SAFETY: the frame is writable and planar float with two planes of at least
            // `capacity` samples, and `samples <= capacity`.
            let (left, right) = unsafe {
                (
                    std::slice::from_raw_parts_mut(raw.data[0].cast::<f32>(), samples),
                    std::slice::from_raw_parts_mut(raw.data[1].cast::<f32>(), samples),
                )
            };
            let (frames, _) = interleaved.as_chunks::<STEREO>();
            for ((&[l, r], left), right) in frames.iter().zip(left).zip(right) {
                *left = l;
                *right = r;
            }
        } else if format == sys::AVSampleFormat::AV_SAMPLE_FMT_FLT as i32 {
            // SAFETY: the frame is writable and packed float with room for `capacity`
            // stereo samples, and `samples <= capacity`.
            let out = unsafe {
                std::slice::from_raw_parts_mut(raw.data[0].cast::<f32>(), samples * STEREO)
            };
            out.copy_from_slice(&interleaved[..samples * STEREO]);
        } else if format == sys::AVSampleFormat::AV_SAMPLE_FMT_S16 as i32 {
            // SAFETY: the frame is writable and packed 16-bit with room for `capacity`
            // stereo samples, and `samples <= capacity`.
            let out = unsafe {
                std::slice::from_raw_parts_mut(raw.data[0].cast::<i16>(), samples * STEREO)
            };
            for (out, &sample) in out.iter_mut().zip(interleaved) {
                *out = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
            }
        } else if format == sys::AVSampleFormat::AV_SAMPLE_FMT_S32 as i32 {
            // SAFETY: as above, for packed 32-bit.
            let out = unsafe {
                std::slice::from_raw_parts_mut(raw.data[0].cast::<i32>(), samples * STEREO)
            };
            for (out, &sample) in out.iter_mut().zip(interleaved) {
                *out = (f64::from(sample.clamp(-1.0, 1.0)) * f64::from(i32::MAX)).round() as i32;
            }
        } else {
            return Err(MediaError::InvalidFrame);
        }
        Ok(())
    }
}
