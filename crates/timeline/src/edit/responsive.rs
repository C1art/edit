use super::ops;
use crate::{ClipId, Edit, PinnedEnds, Sequence, TimelineError};

/// Sets how much of a clip's start and end keeps its keyframes there through trims.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetPinnedEnds {
    pub clip: ClipId,
    pub pinned: PinnedEnds,
}

impl Edit for SetPinnedEnds {
    fn name(&self) -> &'static str {
        "Responsive Design – Time"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let clip = ops::clip_mut(sequence, self.clip)?;
        if clip.pinned == self.pinned {
            return Err(TimelineError::NothingToEdit);
        }
        clip.pinned = self.pinned;
        Ok(())
    }
}
