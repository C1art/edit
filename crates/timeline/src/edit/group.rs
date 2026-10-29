use crate::edit::ops;
use crate::{ClipId, Edit, GroupId, Sequence, TimelineError};

/// Groups `clips` with each other and with whatever they're already grouped or linked
/// with, or breaks up the groups they're in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetGrouped {
    pub clips: Vec<ClipId>,
    pub grouped: bool,
}

impl SetGrouped {
    /// Whether applying this would form a group or break one up.
    pub fn changes(&self, sequence: &Sequence) -> bool {
        let clips = sequence.with_groups(&self.clips);
        let mut groups = clips
            .iter()
            .filter_map(|&id| sequence.clip(id))
            .map(|(_, clip)| clip.group);
        if self.grouped {
            let first = groups.next().flatten();
            let one_already = first.is_some() && groups.all(|group| group == first);
            clips.len() >= 2 && !one_already
        } else {
            groups.any(|group| group.is_some())
        }
    }
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
        if !self.changes(sequence) {
            return Err(TimelineError::NothingToEdit);
        }
        let group = self.grouped.then(|| GroupId(sequence.ids.next()));
        for id in sequence.with_groups(&self.clips) {
            ops::clip_mut(sequence, id)?.group = group;
        }
        Ok(())
    }
}
