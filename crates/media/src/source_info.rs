use timeline::{ColorSpace, FieldOrder, FrameRate, PixelAspect, Ticks};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceInfo {
    /// In the reader's output timebase; `None` when the container doesn't declare it.
    pub duration: Option<Ticks>,
    pub video: Option<VideoInfo>,
    pub has_audio: bool,
    /// A single picture rather than moving video.
    pub still: bool,
    /// Numbered pictures read as one video.
    pub sequence: bool,
    /// The timecode of the first frame, in frames at the video's rate, when the file
    /// carries one.
    pub timecode: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub frame_rate: Option<FrameRate>,
    pub pixel_aspect: PixelAspect,
    pub color_space: ColorSpace,
    pub field_order: FieldOrder,
}

/// Which of a file's streams a reader decodes. Timeline playback opens one reader per
/// stream kind so each can seek on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Streams {
    VideoAndAudio,
    Video,
    Audio,
}
