use std::path::Path;

use ffmpeg_sys_next as sys;

use crate::{ffi, MediaError};

/// Lossless image formats a single frame can be saved in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StillFormat {
    Png,
    Tiff,
    /// Digital Picture Exchange, the film scanning and finishing format.
    Dpx,
}

impl StillFormat {
    pub const ALL: [Self; 3] = [Self::Png, Self::Tiff, Self::Dpx];

    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Tiff => "tif",
            Self::Dpx => "dpx",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Png => "PNG",
            Self::Tiff => "TIFF",
            Self::Dpx => "DPX",
        }
    }

    pub fn of_path(path: &Path) -> Option<Self> {
        let extension = path.extension()?.to_str()?.to_ascii_lowercase();
        match extension.as_str() {
            "png" => Some(Self::Png),
            "tif" | "tiff" => Some(Self::Tiff),
            "dpx" => Some(Self::Dpx),
            _ => None,
        }
    }

    fn codec(self) -> sys::AVCodecID {
        match self {
            Self::Png => sys::AVCodecID::AV_CODEC_ID_PNG,
            Self::Tiff => sys::AVCodecID::AV_CODEC_ID_TIFF,
            Self::Dpx => sys::AVCodecID::AV_CODEC_ID_DPX,
        }
    }
}

/// Saves display-encoded RGBA8 pixels, `size.0 * 4` bytes a row, as an image file.
pub fn write_still(
    path: &Path,
    format: StillFormat,
    size: (u32, u32),
    rgba: &[u8],
) -> Result<(), MediaError> {
    let file = ffi::encode_rgba(format.codec(), size, rgba)?;
    std::fs::write(path, file).map_err(|error| MediaError::CannotWrite {
        path: path.to_owned(),
        reason: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use timeline::Timebase;

    use super::*;
    use crate::{DecodedFrame, MediaReader};

    #[test]
    #[ignore = "encodes and decodes real files with FFmpeg"]
    fn a_still_reads_back_with_the_pixels_it_was_given() {
        // Three pixels make rows narrower than FFmpeg's padded line size.
        let size = (3, 2);
        let rgba: Vec<u8> = (0..24).map(|value| value * 10).collect();
        let directory = std::env::temp_dir();
        for format in StillFormat::ALL {
            let path = directory.join(format!("edit-still-test.{}", format.extension()));
            write_still(&path, format, size, &rgba).unwrap();
            let mut reader = MediaReader::open(&path, Timebase::HZ_90K).unwrap();
            let Some(DecodedFrame::Video(frame)) = reader.next_frame().unwrap() else {
                panic!("{} has no picture", format.name());
            };
            std::fs::remove_file(&path).unwrap();
            assert_eq!((frame.width, frame.height), size);
            assert_eq!(frame.pixels, rgba, "{}", format.name());
        }
    }
}
