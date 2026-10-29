use crate::{Captions, Edit, Sequence, TimelineError};

/// Replaces the sequence's captions: their text, timing, style or visibility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetCaptions {
    pub captions: Captions,
}

impl Edit for SetCaptions {
    fn name(&self) -> &'static str {
        "Edit Captions"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        self.captions.validate()?;
        if sequence.captions == self.captions {
            return Err(TimelineError::NothingToEdit);
        }
        sequence.captions = self.captions.clone();
        Ok(())
    }
}
