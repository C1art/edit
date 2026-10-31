use std::fmt;
use std::sync::{Mutex, PoisonError};

use ffmpeg_sys_next as sys;
use timeline::{
    ChromaSiting, ChromaSubsampling, PlaneFormat, SampleDepth, Ticks, YuvColor, YuvMatrix, YuvRange,
};

use super::{Picture, Scaler};
use crate::{MediaError, VideoFrame};

/// A decoded picture left in the decoder's planes, to be turned into RGB on the GPU,
/// where it costs next to nothing.
pub struct PlanarFrame {
    /// In the reader's output timebase, relative to the start of the media.
    pub pts: Ticks,
    pub width: u32,
    pub height: u32,
    pub color: YuvColor,
    pub format: PlaneFormat,
    layout: Layout,
    picture: Picture,
    #[cfg(windows)]
    surface: Option<super::GpuSurface>,
}

/// One plane's rows, `stride` bytes apart, each starting with `width` samples or, for
/// interleaved chroma, pairs of samples.
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

/// Scalers lent to whichever thread converts, rather than kept per thread: a thread's
/// own would be freed as it exits, and freeing a threaded swscale context joins its
/// slice threads, which on Windows deadlocks under the loader lock that thread-local
/// destructors run in. Statics are never dropped, so these are never freed that way.
static SCALERS: Mutex<Vec<Scaler>> = Mutex::new(Vec::new());

/// A scaler already set up for `picture` if one is idle, else any idle one or a new one.
fn borrow_scaler(picture: &Picture) -> Scaler {
    let mut idle = SCALERS.lock().unwrap_or_else(PoisonError::into_inner);
    let ready = idle.iter().position(|scaler| scaler.is_set_up_for(picture));
    match ready {
        Some(index) => idle.swap_remove(index),
        None => idle.pop().unwrap_or_default(),
    }
}

fn return_scaler(scaler: Scaler) {
    SCALERS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(scaler);
}

impl PlanarFrame {
    /// The picture as a planar frame if its planes and color are ones the GPU path
    /// converts, else the picture back. One on the GPU stays there as a surface where it
    /// can, and is read back where it can't.
    pub(crate) fn new(pts: Ticks, picture: Picture) -> Result<Self, Picture> {
        #[cfg(windows)]
        let surface = super::GpuSurface::of(&picture);
        #[cfg(windows)]
        let on_surface = surface.is_some() && Self::fits_surface(&picture);
        #[cfg(not(windows))]
        let on_surface = false;
        let picture = if picture.on_gpu() && !on_surface {
            match picture.to_memory() {
                Ok(copy) => copy,
                Err(_) => return Err(picture),
            }
        } else {
            picture
        };
        match Self::layout(&picture) {
            Some((layout, format, color, (width, height))) => Ok(Self {
                pts,
                width,
                height,
                color,
                format,
                layout,
                picture,
                #[cfg(windows)]
                surface: surface.filter(|_| on_surface),
            }),
            None => Err(picture),
        }
    }

    /// Whether the GPU path draws straight from this picture's texture: NV12 or P010,
    /// which open as textures with two planes.
    #[cfg(windows)]
    fn fits_surface(picture: &Picture) -> bool {
        use sys::AVPixelFormat::*;
        matches!(picture.format(), AV_PIX_FMT_NV12 | AV_PIX_FMT_P010LE)
            && yuv_color(picture).is_some()
    }

    fn layout(picture: &Picture) -> Option<(Layout, PlaneFormat, YuvColor, (u32, u32))> {
        use sys::AVPixelFormat::*;
        use ChromaSubsampling::*;
        use SampleDepth::*;
        let (layout, subsampling, depth) = match picture.format() {
            AV_PIX_FMT_YUV420P | AV_PIX_FMT_YUVJ420P => (Layout::Separate, Yuv420, Eight),
            AV_PIX_FMT_NV12 => (Layout::Interleaved, Yuv420, Eight),
            AV_PIX_FMT_YUV422P | AV_PIX_FMT_YUVJ422P => (Layout::Separate, Yuv422, Eight),
            AV_PIX_FMT_YUV444P | AV_PIX_FMT_YUVJ444P => (Layout::Separate, Yuv444, Eight),
            AV_PIX_FMT_YUV420P10LE => (Layout::Separate, Yuv420, Ten),
            AV_PIX_FMT_YUV422P10LE => (Layout::Separate, Yuv422, Ten),
            AV_PIX_FMT_YUV444P10LE => (Layout::Separate, Yuv444, Ten),
            AV_PIX_FMT_YUV420P12LE => (Layout::Separate, Yuv420, Twelve),
            AV_PIX_FMT_YUV422P12LE => (Layout::Separate, Yuv422, Twelve),
            AV_PIX_FMT_YUV444P12LE => (Layout::Separate, Yuv444, Twelve),
            AV_PIX_FMT_YUV420P16LE => (Layout::Separate, Yuv420, Sixteen),
            // Ten bits at the top of each word read the same as sixteen.
            AV_PIX_FMT_P010LE | AV_PIX_FMT_P016LE => (Layout::Interleaved, Yuv420, Sixteen),
            AV_PIX_FMT_YUV422P16LE => (Layout::Separate, Yuv422, Sixteen),
            AV_PIX_FMT_YUV444P16LE => (Layout::Separate, Yuv444, Sixteen),
            _ => return None,
        };
        let format = PlaneFormat { subsampling, depth };
        let raw = picture.frame().raw();
        let width = u32::try_from(raw.width).ok().filter(|&width| width > 0)?;
        let height = u32::try_from(raw.height)
            .ok()
            .filter(|&height| height > 0)?;
        let (chroma_width, _) = subsampling.chroma_size((width, height));
        let row_bytes = match layout {
            Layout::Separate => [width, chroma_width, chroma_width],
            Layout::Interleaved => [width, chroma_width * 2, 0],
        }
        .map(|samples| samples * depth.bytes());
        // A picture on the GPU has no planes in memory to check.
        let planes_fit = picture.on_gpu()
            || row_bytes.iter().enumerate().all(|(index, &bytes)| {
                bytes == 0
                    || (!raw.data[index].is_null()
                        && u32::try_from(raw.linesize[index]).is_ok_and(|stride| stride >= bytes))
            });
        let color = yuv_color(picture)?;
        planes_fit.then_some((layout, format, color, (width, height)))
    }

    /// The decoder's texture, when the picture stayed on the GPU, in which case it has
    /// no [`planes`](Self::planes) in memory.
    #[cfg(windows)]
    pub fn surface(&self) -> Option<&super::GpuSurface> {
        self.surface.as_ref()
    }

    /// `None` for a picture on the GPU.
    pub fn planes(&self) -> Option<Planes<'_>> {
        if self.picture.on_gpu() {
            return None;
        }
        let (width, height) = (self.width, self.height);
        let (chroma_width, chroma_height) = self.format.subsampling.chroma_size((width, height));
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
        Some(Planes { luma, chroma })
    }

    fn plane(&self, index: usize, width: u32, height: u32, samples: u32) -> Plane<'_> {
        let raw = self.picture.frame().raw();
        let stride = raw.linesize[index].unsigned_abs();
        let row_bytes = width * samples * self.format.depth.bytes();
        let length = stride as usize * (height as usize - 1) + row_bytes as usize;
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
        let downloaded = if self.picture.on_gpu() {
            Some(self.picture.to_memory()?)
        } else {
            None
        };
        let picture = downloaded.as_ref().unwrap_or(&self.picture);
        let mut scaler = borrow_scaler(picture);
        let converted = scaler.convert(picture);
        return_scaler(scaler);
        let (width, height, pixels) = converted?;
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
            .field("format", &self.format)
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
    let jpeg = matches!(
        picture.format(),
        sys::AVPixelFormat::AV_PIX_FMT_YUVJ420P
            | sys::AVPixelFormat::AV_PIX_FMT_YUVJ422P
            | sys::AVPixelFormat::AV_PIX_FMT_YUVJ444P
    );
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
