use serde::{Deserialize, Serialize};

use crate::{AudioEffect, Clip, ClipId, Param, Sequence, TrackKind, VideoEffect};

/// Effects saved from one clip to apply to others. Keyframes keep their distance from
/// the clip's start, so they land as far into whichever clip gets the preset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectPreset {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub video: Vec<VideoEffect>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub audio: Vec<AudioEffect>,
}

impl EffectPreset {
    /// The effects of `clip` and the clips linked to it, or `None` when they have none.
    pub fn from_clip(name: String, sequence: &Sequence, clip: ClipId) -> Option<Self> {
        let mut preset = Self {
            name,
            video: Vec::new(),
            audio: Vec::new(),
        };
        for id in sequence.with_links(&[clip]) {
            let Some((track, clip)) = sequence.clip(id) else {
                continue;
            };
            let from_clip = |param: &mut Param| {
                for key in &mut param.keys {
                    key.time = clip.position_of(key.time) - clip.start;
                }
            };
            match track.kind {
                TrackKind::Video => {
                    let mut effects = clip.look.effects.clone();
                    effects
                        .iter_mut()
                        .flat_map(VideoEffect::params_mut)
                        .for_each(from_clip);
                    preset.video.extend(effects);
                }
                TrackKind::Audio => {
                    let mut effects = clip.audio_effects.clone();
                    effects
                        .iter_mut()
                        .flat_map(AudioEffect::params_mut)
                        .for_each(from_clip);
                    preset.audio.extend(effects);
                }
            }
        }
        (!preset.is_empty()).then_some(preset)
    }

    pub fn is_empty(&self) -> bool {
        self.video.is_empty() && self.audio.is_empty()
    }

    /// Adds the preset's effects of `kind` after `clip`'s own.
    pub(crate) fn apply(&self, kind: TrackKind, clip: &mut Clip) {
        let timing = clip.clone();
        let onto_clip = |param: &mut Param| {
            for key in &mut param.keys {
                key.time = timing.source_time(timing.start + key.time);
            }
            param.keys.sort_by_key(|key| key.time);
            param.keys.dedup_by_key(|key| key.time);
        };
        match kind {
            TrackKind::Video => {
                let mut effects = self.video.clone();
                effects
                    .iter_mut()
                    .flat_map(VideoEffect::params_mut)
                    .for_each(onto_clip);
                clip.look.effects.extend(effects);
            }
            TrackKind::Audio => {
                let mut effects = self.audio.clone();
                effects
                    .iter_mut()
                    .flat_map(AudioEffect::params_mut)
                    .for_each(onto_clip);
                clip.audio_effects.extend(effects);
            }
        }
    }
}
