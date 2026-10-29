use serde::{Deserialize, Serialize};

use crate::{Clip, ClipId, LinkId, Sequence, Ticks};

/// Whether picking a clip picks the clips linked to it as well, as Premiere's Linked
/// Selection. Apart, each is moved, trimmed and deleted alone, and may slip out of sync.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkedSelection {
    #[default]
    Together,
    Apart,
}

impl Sequence {
    /// `ids` plus, while linked selection is on, every clip linked to one of them,
    /// without duplicates.
    pub fn with_links(&self, ids: &[ClipId]) -> Vec<ClipId> {
        match self.linked_selection {
            LinkedSelection::Together => self.links_of(ids),
            LinkedSelection::Apart => self.clips_among(ids, &[]),
        }
    }

    /// `ids` plus every clip linked to one of them, whether or not linked selection is on.
    pub fn links_of(&self, ids: &[ClipId]) -> Vec<ClipId> {
        let links: Vec<LinkId> = ids.iter().filter_map(|&id| self.clip(id)?.1.link).collect();
        self.clips_among(ids, &links)
    }

    fn clips_among(&self, ids: &[ClipId], links: &[LinkId]) -> Vec<ClipId> {
        let mut all: Vec<ClipId> = self
            .tracks()
            .flat_map(|(_, track)| &track.clips)
            .filter(|clip| ids.contains(&clip.id) || clip.link.is_some_and(|l| links.contains(&l)))
            .map(|clip| clip.id)
            .collect();
        all.sort_unstable();
        all
    }

    /// How much later `clip` plays its media than a clip linked to it plays the same
    /// media; `None` while they are in sync.
    pub fn sync_offset(&self, clip: &Clip) -> Option<Ticks> {
        self.out_of_sync_with(clip).map(|(_, offset)| offset)
    }

    /// The linked clip `clip` is out of sync with, and by how much.
    pub(crate) fn out_of_sync_with(&self, clip: &Clip) -> Option<(&Clip, Ticks)> {
        let link = clip.link?;
        self.tracks()
            .flat_map(|(_, track)| &track.clips)
            .filter(|other| other.id != clip.id && other.link == Some(link))
            .filter(|other| other.source == clip.source)
            .map(|other| (other, clip.start - other.position_of(clip.source_in)))
            .find(|&(_, offset)| offset != Ticks::ZERO)
    }
}
