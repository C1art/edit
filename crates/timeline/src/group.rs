use serde::{Deserialize, Serialize};

use crate::{ClipId, Sequence};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GroupId(pub u64);

impl Sequence {
    /// The clips `ids`, the clips grouped with them, and every clip linked to any of
    /// those: all that a click on one of them selects.
    pub fn with_groups(&self, ids: &[ClipId]) -> Vec<ClipId> {
        let linked = self.with_links(ids);
        let groups: Vec<GroupId> = linked
            .iter()
            .filter_map(|&id| self.clip(id)?.1.group)
            .collect();
        if groups.is_empty() {
            return linked;
        }
        let members: Vec<ClipId> = self
            .tracks()
            .flat_map(|(_, track)| &track.clips)
            .filter(|clip| clip.group.is_some_and(|group| groups.contains(&group)))
            .map(|clip| clip.id)
            .chain(linked)
            .collect();
        self.with_links(&members)
    }
}
