use std::cell::RefCell;
use std::fmt;

use ffmpeg_sys_next as sys;
use timeline::Ticks;

use super::{Picture, Scaler};
use crate::{MediaError, VideoFrame};

/// A decoded picture left in the decoder's 8-bit 4:2:0 planes, to be turned into RGB
/// on the GPU, where it costs next to nothing.
pub struct PlanarFrame {
    /// In the reader's output timebase, relative to the start of the media.
    pub pts: Ticks,
    pub width: u32,
    pub height: u32,
    pub color: YuvColor,
    layout: Layout,
    picture: Picture,
}

/// How a picture's luma and chroma become RGB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YuvColor {
    pub matrix: YuvMatrix,
    pub range: YuvRange,
    pub siting: ChromaSiting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YuvMatrix {
    Bt601,
    Bt709,
    Bt2020,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YuvRange {
    /// Luma from 16 to 235 and chroma from 16 to 240, as video has it.
    Limited,
    Full,
}

/// Where each chroma sample sits between the two luma samples of a row it covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromaSiting {
    /// On the left one, as MPEG-2, H.264 and HEVC place it.
    Left,
    /// Halfway between them, as JPEG places it.
    Center,
}

/// One plane's rows, `stride` bytes apart, each starting with `width` samples.
#[derive(Debug, Clone, Copy)]
pub struct Plane<'a> {
    pub bytes: &'a [u8],
    pub stride: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy)]
pub enum Chroma<'a> {
    /// The blue-difference plane, then the red-difference plane.
    Separate(Plane<'a>, Plane<'a>),
    /// Blue- and red-difference pairs in one plane, as NV12 lays them out.
    Interleaved(Plane<'a>),
}

#[derive(Debug, Clone, Copy)]
pub struct Planes<'a> {
    pub luma: Plane<'a>,
    pub chroma: Chroma<'a>,
}

#[derive(Clone, Copy)]
enum Layout {
    Separate,
    Interleaved,
}

thread_local! {
    static SCALER: RefCell<Scaler> = RefCell::default();
}

impl PlanarFrame {
    /// The picture as a planar frame if it is 8-bit 4:2:0 in a color the GPU path
    /// converts, else the picture back.
    pub(crate) fn new(pts: Ticks, picture: Picture) -> Result<Self, Picture> {
        match Self::layout(&picture) {
            Some((layout, color, width, height)) => Ok(Self {
                pts,
                width,
                height,
                color,
                layout,
                picture,
            }),
            None => Err(picture),
        }
    }

    fn layout(picture: &Picture) -> Option<(Layout, YuvColor, u32, u32)> {
        use sys::AVPixelFormat::*;
        let layout = match picture.format() {
            AV_PIX_FMT_YUV420P | AV_PIX_FMT_YUVJ420P => Layout::Separate,
            AV_PIX_FMT_NV12 => Layout::Interleaved,
            _ => return None,
        };
        let raw = picture.frame().raw();
        let width = u32::try_from(raw.width).ok().filter(|&width| width > 0)?;
        let height = u32::try_from(raw.height)
            .ok()
            .filter(|&height| height > 0)?;
        let chroma_width = width.div_ceil(2);
        let row_bytes = match layout {
            Layout::Separate => [width, chroma_width, chroma_width],
            Layout::Interleaved => [width, chroma_width * 2, 0],
        };
        let planes_fit = row_bytes.iter().enumerate().all(|(index, &bytes)| {
            bytes == 0
                || (!raw.data[index].is_null()
                    && u32::try_from(raw.linesize[index]).is_ok_and(|stride| stride >= bytes))
        });
        let color = yuv_color(picture)?;
        planes_fit.then_some((layout, color, width, height))
    }

    pub fn planes(&self) -> Planes<'_> {
        let (width, height) = (self.width, self.height);
        let (chroma_width, chroma_height) = (width.div_ceil(2), height.div_ceil(2));
        let luma = self.plane(0, width, height, 1);
        let chroma = match self.layout {
            Layout::Separate => Chroma::Separate(
                self.plane(1, chroma_width, chroma_height, 1),
                self.plane(2, chroma_width, chroma_height, 1),
            ),
            Layout::Interleaved => {
                Chroma::Interleaved(self.plane(1, chroma_width, chroma_height, 2))
            }
        };
        Planes { luma, chroma }
    }

    fn plane(&self, index: usize, width: u32, height: u32, samples: u32) -> Plane<'_> {
        let raw = self.picture.frame().raw();
        let stride = raw.linesize[index].unsigned_abs();
        let length = stride as usize * (height as usize - 1) + (width * samples) as usize;
        // SAFETY: `new` checked the plane's pointer is set and its stride covers a row.
        // The decoder's buffers hold `height` rows of `stride` bytes from that pointer,
        // and the frame keeps them alive and unchanged for as long as `self` lives;
        // `length` ends with the last row's samples.
        let bytes = unsafe { std::slice::from_raw_parts(raw.data[index], length) };
        Plane {
            bytes,
            stride,
            width,
            height,
        }
    }

    /// The picture as tightly packed RGBA8, converted on this thread.
    pub fn to_rgba(&self) -> Result<VideoFrame, MediaError> {
        let (width, height, pixels) =
            SCALER.with_borrow_mut(|scaler| scaler.convert(&self.picture))?;
        Ok(VideoFrame {
            pts: self.pts,
            width,
            height,
            pixels,
        })
    }
}

impl fmt::Debug for PlanarFrame {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PlanarFrame")
            .field("pts", &self.pts)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("color", &self.color)
            .finish_non_exhaustive()
    }
}

/// The same matrix the RGBA conversion picks, or `None` for one only it handles.
fn yuv_color(picture: &Picture) -> Option<YuvColor> {
    use sys::AVColorSpace::*;
    let raw = picture.frame().raw();
    let matrix = match raw.colorspace {
        AVCOL_SPC_BT709 => YuvMatrix::Bt709,
        AVCOL_SPC_BT2020_NCL | AVCOL_SPC_BT2020_CL => YuvMatrix::Bt2020,
        AVCOL_SPC_SMPTE170M | AVCOL_SPC_BT470BG => YuvMatrix::Bt601,
        AVCOL_SPC_SMPTE240M | AVCOL_SPC_FCC => return None,
        _ if raw.height >= 720 => YuvMatrix::Bt709,
        _ => YuvMatrix::Bt601,
    };
    let jpeg = picture.format() == sys::AVPixelFormat::AV_PIX_FMT_YUVJ420P;
    let range = if jpeg || raw.color_range == sys::AVColorRange::AVCOL_RANGE_JPEG {
        YuvRange::Full
    } else {
        YuvRange::Limited
    };
    let siting = match raw.chroma_location {
        sys::AVChromaLocation::AVCHROMA_LOC_CENTER => ChromaSiting::Center,
        sys::AVChromaLocation::AVCHROMA_LOC_UNSPECIFIED if jpeg => ChromaSiting::Center,
        _ => ChromaSiting::Left,
    };
    Some(YuvColor {
        matrix,
        range,
        siting,
    })
}
