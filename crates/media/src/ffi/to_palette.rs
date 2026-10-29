use ffmpeg_sys_next as sys;

use super::{Encoder, Frame, RgbaToYuv};
use crate::quantize::Quantizer;
use crate::{MediaError, VideoCodec, VideoSettings};

/// Turns export pictures into what the encoder takes.
pub(crate) enum Converter {
    Yuv(RgbaToYuv),
    Palette(Box<RgbaToPalette>),
}

impl Converter {
    pub(crate) fn new(settings: &VideoSettings, encoder: &Encoder) -> Result<Self, MediaError> {
        let (input, output) = (settings.input_size, (settings.width, settings.height));
        Ok(match settings.codec {
            VideoCodec::Gif => Self::Palette(Box::new(RgbaToPalette::new(input, output)?)),
            _ => Self::Yuv(RgbaToYuv::new(input, output, encoder.pixel_format())?),
        })
    }

    pub(crate) fn convert(&mut self, rgba: &[u8], frame: &mut Frame) -> Result<(), MediaError> {
        match self {
            Self::Yuv(converter) => converter.convert(rgba, frame),
            Self::Palette(converter) => converter.convert(rgba, frame),
        }
    }
}

/// Converts sRGB-encoded RGBA8 to 8-bit palette pictures, each with its own 256
/// colors, scaling it to the encoded size first when that differs.
pub(crate) struct RgbaToPalette {
    scaler: Option<(RgbaToYuv, Frame)>,
    scaled: Vec<u8>,
    indices: Vec<u8>,
    quantizer: Quantizer,
    size: (usize, usize),
}

impl RgbaToPalette {
    pub(crate) fn new(input: (u32, u32), output: (u32, u32)) -> Result<Self, MediaError> {
        let rgba = sys::AVPixelFormat::AV_PIX_FMT_RGBA;
        let scaler = (input != output)
            .then(|| {
                Ok::<_, MediaError>((
                    RgbaToYuv::new(input, output, rgba)?,
                    Frame::for_video(output, rgba)?,
                ))
            })
            .transpose()?;
        let size = (output.0 as usize, output.1 as usize);
        Ok(Self {
            scaler,
            scaled: Vec::new(),
            indices: vec![0; size.0 * size.1],
            quantizer: Quantizer::default(),
            size,
        })
    }

    pub(crate) fn convert(&mut self, rgba: &[u8], frame: &mut Frame) -> Result<(), MediaError> {
        let (width, height) = self.size;
        let picture = match &mut self.scaler {
            Some((scaler, scaled)) => {
                scaler.convert(rgba, scaled)?;
                scaled.packed_rows(width * 4, height, &mut self.scaled)?;
                &self.scaled
            }
            None => rgba,
        };
        if picture.len() != width * height * 4 {
            return Err(MediaError::InvalidFrame);
        }
        self.quantizer.quantize(picture, width, &mut self.indices);
        frame.make_writable()?;
        let raw = frame.raw_mut();
        let stride = usize::try_from(raw.linesize[0]).map_err(|_| MediaError::InvalidFrame)?;
        if (raw.width as usize, raw.height as usize) != self.size || stride < width {
            return Err(MediaError::InvalidFrame);
        }
        // SAFETY: the frame is a writable PAL8 picture of `width × height`, whose first
        // plane holds `height` rows of `stride` bytes and whose second is the 256-entry
        // palette FFmpeg allocates with it.
        let (pixels, palette) = unsafe {
            (
                std::slice::from_raw_parts_mut(raw.data[0], stride * height),
                std::slice::from_raw_parts_mut(raw.data[1].cast::<u32>(), 256),
            )
        };
        for (row, indices) in pixels.chunks_mut(stride).zip(self.indices.chunks(width)) {
            row[..width].copy_from_slice(indices);
        }
        palette.fill(0xff00_0000);
        for (entry, &[r, g, b]) in palette.iter_mut().zip(&self.quantizer.palette) {
            *entry = 0xff00_0000 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b);
        }
        Ok(())
    }
}

impl Frame {
    /// Copies the first plane, `rows` of `row_bytes` each, without its padding.
    fn packed_rows(
        &self,
        row_bytes: usize,
        rows: usize,
        out: &mut Vec<u8>,
    ) -> Result<(), MediaError> {
        let raw = self.raw();
        let stride = usize::try_from(raw.linesize[0]).map_err(|_| MediaError::InvalidFrame)?;
        if stride < row_bytes || (raw.height as usize) < rows {
            return Err(MediaError::InvalidFrame);
        }
        // SAFETY: the frame owns a picture whose first plane holds at least `rows` rows of
        // `stride` bytes, checked against its height and linesize above.
        let plane = unsafe { std::slice::from_raw_parts(raw.data[0], stride * rows) };
        out.clear();
        out.extend(plane.chunks(stride).flat_map(|row| &row[..row_bytes]));
        Ok(())
    }
}
