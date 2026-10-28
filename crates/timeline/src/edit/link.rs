use crate::edit::ops;
use crate::{ClipId, Edit, LinkId, Sequence, TimelineError};

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
        let clips = sequence.with_links(&self.clips);
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
