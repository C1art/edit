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
