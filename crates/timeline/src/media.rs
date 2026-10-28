use std::ops::Range;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::generator::placed_length;
use crate::{BinId, FrameRate, Generator, Nest, Speed, Ticks, Timebase, VideoFormat};

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
    #[serde(default, skip_serializing_if = "MediaLog::is_empty")]
    pub log: MediaLog,
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
    /// What an edit places when no range is marked: all of a file or subclip, or a
    /// still's usual length of generated media.
    pub fn default_range(&self, timebase: Timebase) -> Range<Ticks> {
        match (&self.generator, &self.subclip) {
            (Some(_), _) => Ticks::ZERO..placed_length(timebase).min(self.duration),
            (None, Some(range)) => range.clone(),
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

    /// The item's picture format as it plays, at its interpreted frame rate.
    pub fn played_format(&self) -> Option<VideoFormat> {
        self.video.map(|video| VideoFormat {
            frame_rate: self.interpreted_rate.unwrap_or(video.frame_rate),
            ..video
        })
    }

    pub fn is_file(&self) -> bool {
        self.generator.is_none() && self.nest.is_none()
    }
}
