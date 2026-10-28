use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{Ticks, VideoFormat};

/// Identifies a media item in the sequence's media pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceId(pub u64);

/// A file imported into the media pool, described by what probing found in it.
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
}
