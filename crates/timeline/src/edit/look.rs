use crate::edit::ops;
use crate::{ClipId, Edge, Edit, Look, Sequence, TimelineError, TrackKind, Transition};

/// Replaces a video clip's look: motion, opacity, crop and effects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetClipLook {
    pub clip: ClipId,
    pub look: Look,
}

impl Edit for SetClipLook {
    fn name(&self) -> &'static str {
        "Effect Controls"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        if sequence
            .clip(self.clip)
            .is_some_and(|(track, _)| track.kind != TrackKind::Video)
        {
            return Err(TimelineError::NothingToEdit);
        }
        let clip = ops::clip_mut(sequence, self.clip)?;
        if clip.look == self.look {
            return Err(TimelineError::NothingToEdit);
        }
        clip.look = self.look.clone();
        Ok(())
    }
}

/// Adds, replaces or removes the transition at one end of a clip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetTransition {
    pub clip: ClipId,
    pub edge: Edge,
    pub transition: Option<Transition>,
}

impl Edit for SetTransition {
    fn name(&self) -> &'static str {
        match self.transition {
            Some(_) => "Apply Transition",
            None => "Remove Transition",
        }
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let clip = ops::clip_mut(sequence, self.clip)?;
        let slot = match self.edge {
            Edge::Head => &mut clip.transition_in,
            Edge::Tail => &mut clip.transition_out,
        };
        if *slot == self.transition {
            return Err(TimelineError::NothingToEdit);
        }
        *slot = self.transition;
        Ok(())
    }
}
