use crate::edit::ops;
use crate::{ClipId, Edit, Sequence, TimelineError};

/// Enables or disables `clips` and the clips linked to them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetClipsEnabled {
    pub clips: Vec<ClipId>,
    pub enabled: bool,
}

impl Edit for SetClipsEnabled {
    fn name(&self) -> &'static str {
        if self.enabled {
            "Enable"
        } else {
            "Disable"
        }
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let mut changed = false;
        for id in sequence.with_links(&self.clips) {
            let clip = ops::clip_mut(sequence, id)?;
            changed |= clip.disabled == self.enabled;
            clip.disabled = !self.enabled;
        }
        if changed {
            Ok(())
        } else {
            Err(TimelineError::NothingToEdit)
        }
    }
}
