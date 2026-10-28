use crate::edit::ops;
use crate::{
    AudioEffect, ClipId, Edit, Gain, Pan, Param, Sequence, TimelineError, TrackKind, TrackRef,
};

/// Sets the gain of the audio clips among `clips` and the clips linked to them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetClipGain {
    pub clips: Vec<ClipId>,
    pub gain: Gain,
}

impl Edit for SetClipGain {
    fn name(&self) -> &'static str {
        "Audio Gain"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let audio: Vec<ClipId> = sequence
            .with_links(&self.clips)
            .into_iter()
            .filter(|&id| {
                sequence
                    .clip(id)
                    .is_some_and(|(track, _)| track.kind == TrackKind::Audio)
            })
            .collect();
        if audio.is_empty() {
            return Err(TimelineError::NothingToEdit);
        }
        for id in audio {
            ops::clip_mut(sequence, id)?.gain = self.gain;
        }
        Ok(())
    }
}

/// Sets each listed audio clip's gain on its own, as normalizing clips to a common
/// peak or loudness does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizeClipGains {
    pub gains: Vec<(ClipId, Gain)>,
}

impl Edit for NormalizeClipGains {
    fn name(&self) -> &'static str {
        "Normalize"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        if self.gains.is_empty() {
            return Err(TimelineError::NothingToEdit);
        }
        for &(id, gain) in &self.gains {
            if sequence
                .clip(id)
                .is_some_and(|(track, _)| track.kind != TrackKind::Audio)
            {
                return Err(TimelineError::NothingToEdit);
            }
            ops::clip_mut(sequence, id)?.gain = gain;
        }
        Ok(())
    }
}

/// Replaces an audio clip's volume level and its keyframes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetClipVolume {
    pub clip: ClipId,
    pub volume: Param,
}

impl Edit for SetClipVolume {
    fn name(&self) -> &'static str {
        "Volume"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        if sequence
            .clip(self.clip)
            .is_some_and(|(track, _)| track.kind != TrackKind::Audio)
        {
            return Err(TimelineError::NothingToEdit);
        }
        let clip = ops::clip_mut(sequence, self.clip)?;
        if clip.volume == self.volume {
            return Err(TimelineError::NothingToEdit);
        }
        clip.volume = self.volume.clone();
        Ok(())
    }
}

/// Replaces an audio clip's effects and their settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetClipAudioEffects {
    pub clip: ClipId,
    pub effects: Vec<AudioEffect>,
}

impl Edit for SetClipAudioEffects {
    fn name(&self) -> &'static str {
        "Audio Effects"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        if sequence
            .clip(self.clip)
            .is_some_and(|(track, _)| track.kind != TrackKind::Audio)
        {
            return Err(TimelineError::NothingToEdit);
        }
        let clip = ops::clip_mut(sequence, self.clip)?;
        if clip.audio_effects == self.effects {
            return Err(TimelineError::NothingToEdit);
        }
        clip.audio_effects.clone_from(&self.effects);
        Ok(())
    }
}

/// Sets an audio track's fader and balance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetTrackMix {
    pub track: TrackRef,
    pub volume: Gain,
    pub pan: Pan,
}

impl Edit for SetTrackMix {
    fn name(&self) -> &'static str {
        "Track Volume"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        if self.track.kind != TrackKind::Audio {
            return Err(TimelineError::NothingToEdit);
        }
        let track = sequence
            .track_mut(self.track)
            .ok_or(TimelineError::TrackNotFound(self.track))?;
        track.volume = self.volume;
        track.pan = self.pan;
        Ok(())
    }
}

/// Replaces an audio track's volume automation and its keyframes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetTrackAutomation {
    pub track: TrackRef,
    pub automation: Param,
}

impl Edit for SetTrackAutomation {
    fn name(&self) -> &'static str {
        "Track Keyframes"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        if self.track.kind != TrackKind::Audio {
            return Err(TimelineError::NothingToEdit);
        }
        let track = sequence
            .track_mut(self.track)
            .ok_or(TimelineError::TrackNotFound(self.track))?;
        if track.automation == self.automation {
            return Err(TimelineError::NothingToEdit);
        }
        track.automation = self.automation.clone();
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetMasterGain {
    pub gain: Gain,
}

impl Edit for SetMasterGain {
    fn name(&self) -> &'static str {
        "Master Volume"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        sequence.master = self.gain;
        Ok(())
    }
}
