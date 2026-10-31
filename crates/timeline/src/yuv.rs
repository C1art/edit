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

/// How a picture's planes hold its samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlaneFormat {
    pub subsampling: ChromaSubsampling,
    pub depth: SampleDepth,
}

/// How many chroma samples there are for the luma samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChromaSubsampling {
    /// Half as many across and down.
    Yuv420,
    /// Half as many across.
    Yuv422,
    /// As many.
    Yuv444,
}

/// The bits in each sample: a byte holds eight, a little-endian 16-bit word the rest,
/// in its low bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SampleDepth {
    Eight,
    Ten,
    Twelve,
    Sixteen,
}

impl ChromaSubsampling {
    /// The chroma planes' size for a `width` by `height` picture.
    pub fn chroma_size(self, (width, height): (u32, u32)) -> (u32, u32) {
        match self {
            Self::Yuv420 => (width.div_ceil(2), height.div_ceil(2)),
            Self::Yuv422 => (width.div_ceil(2), height),
            Self::Yuv444 => (width, height),
        }
    }
}

impl SampleDepth {
    pub fn bits(self) -> u32 {
        match self {
            Self::Eight => 8,
            Self::Ten => 10,
            Self::Twelve => 12,
            Self::Sixteen => 16,
        }
    }

    pub fn bytes(self) -> u32 {
        match self {
            Self::Eight => 1,
            Self::Ten | Self::Twelve | Self::Sixteen => 2,
        }
    }
}
