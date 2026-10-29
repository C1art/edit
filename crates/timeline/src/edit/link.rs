use crate::edit::ops;
use crate::{ClipId, Edit, LinkId, LinkedSelection, Sequence, TimelineError};

/// Links `clips` and the clips already linked to them so they move, trim and delete
/// together, or unlinks all of those from one another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetLinked {
    pub clips: Vec<ClipId>,
    pub linked: bool,
}

impl Edit for SetLinked {
    fn name(&self) -> &'static str {
        if self.linked {
            "Link"
        } else {
            "Unlink"
        }
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let clips = sequence.links_of(&self.clips);
        let mut links = Vec::new();
        for &id in &clips {
            links.push(
                sequence
                    .clip(id)
                    .ok_or(TimelineError::ClipNotFound(id))?
                    .1
                    .link,
            );
        }
        let link = if self.linked {
            let shared = links.first().copied().flatten();
            if clips.len() < 2 || links.iter().all(|&link| link.is_some() && link == shared) {
                return Err(TimelineError::NothingToEdit);
            }
            Some(LinkId(sequence.ids.next()))
        } else {
            if links.iter().all(Option::is_none) {
                return Err(TimelineError::NothingToEdit);
            }
            None
        };
        for id in clips {
            ops::clip_mut(sequence, id)?.link = link;
        }
        Ok(())
    }
}

/// Turns Premiere's Linked Selection on or off for the sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetLinkedSelection(pub LinkedSelection);

impl Edit for SetLinkedSelection {
    fn name(&self) -> &'static str {
        "Linked Selection"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        if sequence.linked_selection == self.0 {
            return Err(TimelineError::NothingToEdit);
        }
        sequence.linked_selection = self.0;
        Ok(())
    }
}

/// Runs an edit as though linked selection were off, so it touches only the clips it
/// names, as an Alt-drag of one side of a linked pair does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unlinked<E>(pub E);

impl<E: Edit> Edit for Unlinked<E> {
    fn name(&self) -> &'static str {
        self.0.name()
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let links = std::mem::replace(&mut sequence.linked_selection, LinkedSelection::Apart);
        let result = self.0.edit(sequence);
        sequence.linked_selection = links;
        result
    }
}
