use std::ops::Range;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::generator::placed_length;
use crate::{
    AlphaInterpretation, BinId, ColorSpace, FieldOrder, FrameFit, FrameRate, Generator, Nest,
    PixelAspect, Speed, Ticks, Timebase, VideoFormat,
};

/// Identifies a media item in the sequence's media pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceId(pub u64);

/// A file imported into the media pool, described by what probing found in it, pictures
/// the editor draws itself, or a nested sequence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaItem {
    pub id: SourceId,
    pub name: String,
    pub path: PathBuf,
    /// In the sequence timebase. Clips can't extend past it.
    pub duration: Ticks,
    pub video: Option<VideoFormat>,
    pub has_audio: bool,
    /// A smaller copy of the pictures for smoother playback; it carries no sound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy: Option<PathBuf>,
    /// Set for media drawn by the editor, which has no file behind `path`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generator: Option<Generator>,
    /// Set for a sequence nested in the project's, which has no file behind `path`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nest: Option<Nest>,
    /// The bin the item sits in, or none at the top of the project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bin: Option<BinId>,
    /// The part of the file a subclip stands for; clips of it start inside this range.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subclip: Option<Range<Ticks>>,
    /// The frame rate the pictures are played at instead of the file's own, as for
    /// slowing high-frame-rate footage; the sound follows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interpreted_rate: Option<FrameRate>,
    /// The pixel shape the pictures are drawn with instead of the file's own, as for
    /// anamorphic footage the file doesn't flag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interpreted_aspect: Option<PixelAspect>,
    #[serde(default, skip_serializing_if = "AlphaInterpretation::is_straight")]
    pub alpha: AlphaInterpretation,
    /// The color space the pictures are read in instead of the one the file declares,
    /// as for footage that is flagged wrongly or not at all.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interpreted_color: Option<ColorSpace>,
    /// The field order the pictures are read with instead of the one the file declares.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interpreted_fields: Option<FieldOrder>,
    #[serde(default, skip_serializing_if = "MediaLog::is_empty")]
    pub log: MediaLog,
    /// Set by Make Offline: the file is left alone but no longer read, until relinked.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub offline: bool,
    /// A single picture, held for however long its clips run.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub still: bool,
    /// The file's channels played on each side instead of all of them mixed to stereo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_map: Option<ChannelMap>,
    /// How long an edit places of a still or generated media when nothing is marked,
    /// as the preference stood when it was imported; unset places the standard length.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placed: Option<Ticks>,
    /// How clips of the pictures are scaled to the sequence frame when placed, as the
    /// preference stood when it was imported; unset places them at their own size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_fit: Option<FrameFit>,
    /// The timecode of the first frame, counted in frames at the rate the item plays:
    /// the file's own, or as Modify > Timecode sets it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timecode: Option<i64>,
}

/// Which of a file's audio channels, counting from zero, play on the left and the right;
/// the same channel on both plays it as mono.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChannelMap {
    pub left: u16,
    pub right: u16,
}

/// What the editor has written down about a media item, as Premiere's Metadata panel
/// keeps it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaLog {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub scene: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub shot: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub log_note: String,
    /// Marked as a good take.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub good: bool,
}

impl MediaLog {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

impl MediaItem {
    /// What an edit places when no range is marked: all of a file or subclip, or the
    /// usual still length of a picture or generated media.
    pub fn default_range(&self, timebase: Timebase) -> Range<Ticks> {
        let placed = || {
            let length = self.placed.unwrap_or_else(|| placed_length(timebase));
            Ticks::ZERO..length.min(self.duration)
        };
        match (&self.generator, &self.subclip) {
            (Some(Generator::CountingLeader(_)), _) => Ticks::ZERO..self.duration,
            (Some(_), _) => placed(),
            (None, Some(range)) => range.clone(),
            (None, None) if self.still => placed(),
            (None, None) => Ticks::ZERO..self.duration,
        }
    }

    /// How much file time passes per second the clips of this item play, from its
    /// interpreted frame rate.
    pub fn conform(&self) -> Speed {
        let (Some(video), Some(assumed)) = (self.video, self.interpreted_rate) else {
            return Speed::NORMAL;
        };
        let native = video.frame_rate;
        Speed::new(
            u64::from(assumed.num()) * u64::from(native.den()),
            u64::from(assumed.den()) * u64::from(native.num()),
        )
        .unwrap_or(Speed::NORMAL)
    }

    /// The item's picture format as it plays: at its interpreted frame rate, and as
    /// wide in square pixels as its own pixels make it look.
    pub fn played_format(&self) -> Option<VideoFormat> {
        self.video.map(|video| VideoFormat {
            width: self.pixel_aspect().display_width(video.width),
            frame_rate: self.interpreted_rate.unwrap_or(video.frame_rate),
            pixel_aspect: PixelAspect::SQUARE,
            ..video
        })
    }

    /// How far into the timecode clock the item's first frame is; zero without one.
    pub fn start_timecode(&self, timebase: Timebase) -> Ticks {
        match (self.timecode, self.played_format()) {
            (Some(frame), Some(format)) => format.frame_rate.frame_start(frame, timebase),
            _ => Ticks::ZERO,
        }
    }

    /// Whether the file is read otherwise than it describes itself.
    pub fn is_interpreted(&self) -> bool {
        self.interpreted_rate.is_some()
            || self.interpreted_aspect.is_some()
            || !self.alpha.is_straight()
            || self.interpreted_color.is_some()
            || self.interpreted_fields.is_some()
    }

    /// The field order the pictures are read with: the interpreted one or the file's.
    pub fn field_order(&self) -> FieldOrder {
        self.interpreted_fields
            .or(self.video.map(|video| video.field_order))
            .unwrap_or_default()
    }

    /// The color space the pictures are read in: the interpreted one or the file's.
    pub fn color_space(&self) -> ColorSpace {
        self.interpreted_color
            .or(self.video.map(|video| video.color_space))
            .unwrap_or_default()
    }

    /// The pixel shape the pictures are drawn with: the interpreted one or the file's.
    pub fn pixel_aspect(&self) -> PixelAspect {
        self.interpreted_aspect
            .or(self.video.map(|video| video.pixel_aspect))
            .unwrap_or_default()
    }

    pub fn is_file(&self) -> bool {
        self.generator.is_none() && self.nest.is_none()
    }
}
