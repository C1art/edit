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

/// Raises or lowers the gain of the audio clips among `clips` and the clips linked to
/// them by `by`, each from where it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NudgeClipGain {
    pub clips: Vec<ClipId>,
    pub by: Gain,
}

impl Edit for NudgeClipGain {
    fn name(&self) -> &'static str {
        "Audio Gain"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let nudged: Vec<(ClipId, Gain)> = sequence
            .with_links(&self.clips)
            .into_iter()
            .filter_map(|id| sequence.clip(id))
            .filter(|(track, _)| track.kind == TrackKind::Audio)
            .map(|(_, clip)| {
                (
                    clip.id,
                    Gain((clip.gain.0 + self.by.0).clamp(Gain::SILENT.0, Gain::MAX.0)),
                )
            })
            .collect();
        if nudged
            .iter()
            .all(|&(id, gain)| sequence.clip(id).is_some_and(|(_, clip)| clip.gain == gain))
        {
            return Err(TimelineError::NothingToEdit);
        }
        for (id, gain) in nudged {
            ops::clip_mut(sequence, id)?.gain = gain;
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

/// Replaces a video clip's opacity and its keyframes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetClipOpacity {
    pub clip: ClipId,
    pub opacity: Param,
}

impl Edit for SetClipOpacity {
    fn name(&self) -> &'static str {
        "Opacity"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        if sequence
            .clip(self.clip)
            .is_some_and(|(track, _)| track.kind != TrackKind::Video)
        {
            return Err(TimelineError::NothingToEdit);
        }
        let clip = ops::clip_mut(sequence, self.clip)?;
        if clip.look.opacity == self.opacity {
            return Err(TimelineError::NothingToEdit);
        }
        clip.look.opacity = self.opacity.clone();
        Ok(())
    }
}

/// Replaces an audio clip's balance and its keyframes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetClipPan {
    pub clip: ClipId,
    pub pan: Param,
}

impl Edit for SetClipPan {
    fn name(&self) -> &'static str {
        "Balance"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        if sequence
            .clip(self.clip)
            .is_some_and(|(track, _)| track.kind != TrackKind::Audio)
        {
            return Err(TimelineError::NothingToEdit);
        }
        let clip = ops::clip_mut(sequence, self.clip)?;
        if clip.pan == self.pan {
            return Err(TimelineError::NothingToEdit);
        }
        clip.pan = self.pan.clone();
        Ok(())
    }
}

/// Gives audio clips volume keyframes that lower them under other sound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoDuck {
    pub volumes: Vec<(ClipId, Param)>,
}

impl Edit for AutoDuck {
    fn name(&self) -> &'static str {
        "Auto Duck"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        if self.volumes.is_empty() {
            return Err(TimelineError::NothingToEdit);
        }
        for (id, volume) in &self.volumes {
            if sequence
                .clip(*id)
                .is_some_and(|(track, _)| track.kind != TrackKind::Audio)
            {
                return Err(TimelineError::NothingToEdit);
            }
            ops::clip_mut(sequence, *id)?.volume = volume.clone();
        }
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

/// Replaces the master's inserts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetMasterEffects {
    pub effects: Vec<AudioEffect>,
}

impl Edit for SetMasterEffects {
    fn name(&self) -> &'static str {
        "Master Effects"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        if sequence.master_effects == self.effects {
            return Err(TimelineError::NothingToEdit);
        }
        sequence.master_effects.clone_from(&self.effects);
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

/// Replaces an audio track's inserts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetTrackEffects {
    pub track: TrackRef,
    pub effects: Vec<AudioEffect>,
}

impl Edit for SetTrackEffects {
    fn name(&self) -> &'static str {
        "Track Effects"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        if self.track.kind != TrackKind::Audio {
            return Err(TimelineError::NothingToEdit);
        }
        let track = sequence
            .track_mut(self.track)
            .ok_or(TimelineError::TrackNotFound(self.track))?;
        if track.effects == self.effects {
            return Err(TimelineError::NothingToEdit);
        }
        track.effects.clone_from(&self.effects);
        Ok(())
    }
}
