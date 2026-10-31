use crate::edit::ops;
use crate::{ClipId, Edit, Look, Param, Sequence, TimelineError, TrackKind};

/// Sets the looks of several video clips and the volumes of several audio clips in one
/// step, as adjusting a selection of clips together does.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AdjustClips {
    pub looks: Vec<(ClipId, Look)>,
    pub volumes: Vec<(ClipId, Param)>,
}

impl Edit for AdjustClips {
    fn name(&self) -> &'static str {
        "Adjust Clips"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let mut changed = false;
        for (id, look) in &self.looks {
            let clip = clip_on(sequence, *id, TrackKind::Video)?;
            changed |= clip.look != *look;
            clip.look.clone_from(look);
        }
        for (id, volume) in &self.volumes {
            let clip = clip_on(sequence, *id, TrackKind::Audio)?;
            changed |= clip.volume != *volume;
            clip.volume.clone_from(volume);
        }
        if changed {
            Ok(())
        } else {
            Err(TimelineError::NothingToEdit)
        }
    }
}

fn clip_on(
    sequence: &mut Sequence,
    id: ClipId,
    kind: TrackKind,
) -> Result<&mut crate::Clip, TimelineError> {
    if sequence
        .clip(id)
        .is_some_and(|(track, _)| track.kind != kind)
    {
        return Err(TimelineError::NothingToEdit);
    }
    ops::clip_mut(sequence, id)
}
