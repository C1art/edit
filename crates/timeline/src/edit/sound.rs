use crate::edit::ops;
use crate::{AudioEffect, AudioType, ClipId, Edit, Sequence, TimelineError, TrackKind};

/// An audio clip's type and effects as Essential Sound leaves them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipSound {
    pub clip: ClipId,
    pub audio_type: Option<AudioType>,
    pub effects: Vec<AudioEffect>,
}

/// Sets the type and effects of several audio clips in one step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetClipsSound {
    pub clips: Vec<ClipSound>,
}

impl Edit for SetClipsSound {
    fn name(&self) -> &'static str {
        "Essential Sound"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let mut changed = false;
        for sound in &self.clips {
            if sequence
                .clip(sound.clip)
                .is_some_and(|(track, _)| track.kind != TrackKind::Audio)
            {
                return Err(TimelineError::NothingToEdit);
            }
            let clip = ops::clip_mut(sequence, sound.clip)?;
            changed |= clip.audio_type != sound.audio_type || clip.audio_effects != sound.effects;
            clip.audio_type = sound.audio_type;
            clip.audio_effects.clone_from(&sound.effects);
        }
        if changed {
            Ok(())
        } else {
            Err(TimelineError::NothingToEdit)
        }
    }
}
