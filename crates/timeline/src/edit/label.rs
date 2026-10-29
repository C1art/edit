use crate::edit::ops;
use crate::{ClipId, Edit, Label, Sequence, TimelineError};

/// Names clips on the timeline apart from their media; an empty name goes back to the
/// media's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameClips {
    pub clips: Vec<ClipId>,
    pub name: String,
}

impl Edit for RenameClips {
    fn name(&self) -> &'static str {
        "Rename Clip"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let name = Some(self.name.trim()).filter(|name| !name.is_empty());
        let mut changed = false;
        for &id in &self.clips {
            let clip = ops::clip_mut(sequence, id)?;
            changed |= clip.name.as_deref() != name;
            clip.name = name.map(str::to_owned);
        }
        if changed {
            Ok(())
        } else {
            Err(TimelineError::NothingToEdit)
        }
    }
}

/// Colors clips and the clips linked to them, or returns them to their track's color.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetClipLabel {
    pub clips: Vec<ClipId>,
    pub label: Option<Label>,
}

impl Edit for SetClipLabel {
    fn name(&self) -> &'static str {
        "Label"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let mut changed = false;
        for id in sequence.with_links(&self.clips) {
            let clip = ops::clip_mut(sequence, id)?;
            changed |= clip.label != self.label;
            clip.label = self.label;
        }
        if changed {
            Ok(())
        } else {
            Err(TimelineError::NothingToEdit)
        }
    }
}
