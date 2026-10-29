use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{Clip, ClipId, LinkId, Sequence, SourceId, Ticks, TrackRef};

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
        sorted_ids(self.clips_with_links(ids))
    }

    /// The clips [`Sequence::with_links`] names, with the tracks they're on, in track order.
    pub fn clips_with_links(&self, ids: &[ClipId]) -> Vec<(TrackRef, &Clip)> {
        self.clips_among(ids, self.linked_selection == LinkedSelection::Together)
    }

    /// `ids` plus every clip linked to one of them, whether or not linked selection is on.
    pub fn links_of(&self, ids: &[ClipId]) -> Vec<ClipId> {
        sorted_ids(self.clips_among(ids, true))
    }

    // Sets rather than slices: a drag of hundreds of clips asks this every frame.
    fn clips_among(&self, ids: &[ClipId], follow_links: bool) -> Vec<(TrackRef, &Clip)> {
        let ids: HashSet<ClipId> = ids.iter().copied().collect();
        let clips = || {
            self.tracks().flat_map(|(track_ref, track)| {
                track.clips.iter().map(move |clip| (track_ref, clip))
            })
        };
        let links: HashSet<LinkId> = if follow_links {
            clips()
                .filter(|(_, clip)| ids.contains(&clip.id))
                .filter_map(|(_, clip)| clip.link)
                .collect()
        } else {
            HashSet::new()
        };
        clips()
            .filter(|(_, clip)| {
                ids.contains(&clip.id) || clip.link.is_some_and(|link| links.contains(&link))
            })
            .collect()
    }

    /// How much later `clip` plays its media than a clip linked to it plays the same
    /// media; `None` while they are in sync.
    pub fn sync_offset(&self, clip: &Clip) -> Option<Ticks> {
        self.out_of_sync_with(clip).map(|(_, offset)| offset)
    }

    /// Every out-of-sync clip's [`Sequence::sync_offset`], found in one pass rather than
    /// by searching the whole sequence for each clip's partners.
    pub fn sync_offsets(&self) -> HashMap<ClipId, Ticks> {
        let mut partners: HashMap<(LinkId, SourceId), Vec<&Clip>> = HashMap::new();
        for clip in self.tracks().flat_map(|(_, track)| &track.clips) {
            if let Some(link) = clip.link {
                partners.entry((link, clip.source)).or_default().push(clip);
            }
        }
        partners
            .values()
            .flat_map(|group| {
                group.iter().filter_map(move |clip| {
                    group
                        .iter()
                        .filter(|other| other.id != clip.id)
                        .map(|other| clip.start - other.position_of(clip.source_in))
                        .find(|&offset| offset != Ticks::ZERO)
                        .map(|offset| (clip.id, offset))
                })
            })
            .collect()
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

fn sorted_ids(clips: Vec<(TrackRef, &Clip)>) -> Vec<ClipId> {
    let mut ids: Vec<ClipId> = clips.into_iter().map(|(_, clip)| clip.id).collect();
    ids.sort_unstable();
    ids
}
