use serde::{Deserialize, Serialize};

use crate::{AudioEffect, Clip, Gain, Pan, Param, SubmixId, Ticks};

/// Clips are kept sorted by `start` and never overlap.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    pub clips: Vec<Clip>,
    /// Shown beside the track's number, as "A1 Dialogue"; none until the user names it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Hidden for video, silent for audio.
    #[serde(default)]
    pub muted: bool,
    /// Edits that would change a locked track's clips are rejected.
    #[serde(default)]
    pub locked: bool,
    /// While any audio track is soloed, only soloed ones are heard.
    #[serde(default)]
    pub solo: bool,
    /// Fader level of an audio track; ignored for video.
    #[serde(default)]
    pub volume: Gain,
    #[serde(default)]
    pub pan: Pan,
    /// An audio track's level in decibels after its fader, keyframed over sequence time
    /// rather than any clip's, so it stays put when clips move.
    #[serde(default)]
    pub automation: Param,
    /// An audio track's inserts, run in order over its clips' mix before the fader, so
    /// they carry on through gaps between clips.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<AudioEffect>,
    /// The submix an audio track is sent to; the master when none, or when the submix
    /// isn't in the sequence mixing it, as after nesting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<SubmixId>,
}

impl Track {
    pub fn clip_at(&self, position: Ticks) -> Option<&Clip> {
        let index = self.clips.partition_point(|clip| clip.start <= position);
        self.clips[..index]
            .last()
            .filter(|clip| clip.covers(position))
    }

    pub fn end(&self) -> Ticks {
        self.clips.last().map_or(Ticks::ZERO, Clip::end)
    }
}
