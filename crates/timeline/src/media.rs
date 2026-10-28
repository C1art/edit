use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::generator::placed_length;
use crate::{BinId, Generator, Nest, Ticks, Timebase, VideoFormat};

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
}

impl MediaItem {
    /// How much an edit places when no range is marked: all of a file, or a still's
    /// usual length of generated media.
    pub fn default_length(&self, timebase: Timebase) -> Ticks {
        match self.generator {
            Some(_) => placed_length(timebase).min(self.duration),
            None => self.duration,
        }
    }

    pub fn is_file(&self) -> bool {
        self.generator.is_none() && self.nest.is_none()
    }
}
