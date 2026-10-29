use crate::edit::ops;
use crate::{ClipId, Edit, GroupId, Sequence, TimelineError};

/// Groups `clips` with each other and with whatever they're already grouped or linked
/// with, or breaks up the groups they're in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetGrouped {
    pub clips: Vec<ClipId>,
    pub grouped: bool,
}

impl Edit for SetGrouped {
    fn name(&self) -> &'static str {
        if self.grouped {
            "Group"
        } else {
            "Ungroup"
        }
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let clips = sequence.with_groups(&self.clips);
        let groups = clips
            .iter()
            .map(|&id| {
                Ok(sequence
                    .clip(id)
                    .ok_or(TimelineError::ClipNotFound(id))?
                    .1
                    .group)
            })
            .collect::<Result<Vec<_>, TimelineError>>()?;
        let group = if self.grouped {
            let first = groups.first().copied().flatten();
            let one_already = first.is_some() && groups.iter().all(|&group| group == first);
            if clips.len() < 2 || one_already {
                return Err(TimelineError::NothingToEdit);
            }
            Some(GroupId(sequence.ids.next()))
        } else {
            if groups.iter().all(Option::is_none) {
                return Err(TimelineError::NothingToEdit);
            }
            None
        };
        for id in clips {
            ops::clip_mut(sequence, id)?.group = group;
        }
        Ok(())
    }
}
