use crate::{Edit, Sequence, Submix, SubmixId, TimelineError, TrackKind, TrackRef, TrackSend};

/// Adds a submix after the existing ones, named for its place among them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddSubmix;

impl Edit for AddSubmix {
    fn name(&self) -> &'static str {
        "Add Submix"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let id = SubmixId(sequence.ids.next());
        let name = format!("Submix {}", sequence.submixes.len() + 1);
        sequence.submixes.push(Submix::new(id, name));
        Ok(())
    }
}

/// Deletes a submix; the tracks sent to it go to the master instead, and their sends
/// to it are dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveSubmix {
    pub submix: SubmixId,
}

impl Edit for RemoveSubmix {
    fn name(&self) -> &'static str {
        "Delete Submix"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let index = sequence
            .submixes
            .iter()
            .position(|submix| submix.id == self.submix)
            .ok_or(TimelineError::NothingToEdit)?;
        sequence.submixes.remove(index);
        sequence.audio.iter_mut().for_each(|track| {
            if track.output == Some(self.submix) {
                track.output = None;
            }
            track.sends.retain(|send| send.to != self.submix);
        });
        Ok(())
    }
}

/// Replaces a submix's name, levels, mute and effects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetSubmix {
    pub submix: Submix,
}

impl Edit for SetSubmix {
    fn name(&self) -> &'static str {
        "Submix"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let submix = sequence
            .submixes
            .iter_mut()
            .find(|submix| submix.id == self.submix.id)
            .ok_or(TimelineError::NothingToEdit)?;
        if *submix == self.submix {
            return Err(TimelineError::NothingToEdit);
        }
        submix.clone_from(&self.submix);
        Ok(())
    }
}

/// Sends an audio track to a submix, or to the master when `output` is none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetTrackOutput {
    pub track: TrackRef,
    pub output: Option<SubmixId>,
}

impl Edit for SetTrackOutput {
    fn name(&self) -> &'static str {
        "Track Output"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let known = self
            .output
            .is_none_or(|id| sequence.submixes.iter().any(|submix| submix.id == id));
        if self.track.kind != TrackKind::Audio || !known {
            return Err(TimelineError::NothingToEdit);
        }
        let track = sequence
            .track_mut(self.track)
            .ok_or(TimelineError::TrackNotFound(self.track))?;
        if track.output == self.output {
            return Err(TimelineError::NothingToEdit);
        }
        track.output = self.output;
        Ok(())
    }
}

/// Replaces an audio track's sends, at most [`TrackSend::MAX`] of them, each to a
/// submix of the sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetTrackSends {
    pub track: TrackRef,
    pub sends: Vec<TrackSend>,
}

impl Edit for SetTrackSends {
    fn name(&self) -> &'static str {
        "Track Sends"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let known = |send: &TrackSend| sequence.submixes.iter().any(|submix| submix.id == send.to);
        let valid = self.track.kind == TrackKind::Audio
            && self.sends.len() <= TrackSend::MAX
            && self.sends.iter().all(known);
        if !valid {
            return Err(TimelineError::NothingToEdit);
        }
        let track = sequence
            .track_mut(self.track)
            .ok_or(TimelineError::TrackNotFound(self.track))?;
        if track.sends == self.sends {
            return Err(TimelineError::NothingToEdit);
        }
        track.sends.clone_from(&self.sends);
        Ok(())
    }
}
