use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::{Clip, Edit, MediaItem, Sequence, SourceId, TimelineError};

/// Takes out of the project every item no clip uses, in any sequence. Sequences stay,
/// as they are what the project is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoveUnused;

impl Edit for RemoveUnused {
    fn name(&self) -> &'static str {
        "Remove Unused"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let used = used_media(sequence);
        let count = sequence.media.len();
        Arc::make_mut(&mut sequence.media)
            .retain(|item| item.nest.is_some() || used.contains(&item.id));
        if sequence.media.len() == count {
            return Err(TimelineError::NothingToEdit);
        }
        Ok(())
    }
}

/// Merges items that are the same footage read the same way into the first of them, so
/// a file imported twice is one item; clips of the others become clips of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConsolidateDuplicates;

impl Edit for ConsolidateDuplicates {
    fn name(&self) -> &'static str {
        "Consolidate Duplicates"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let merged = duplicates(&sequence.media);
        if merged.is_empty() {
            return Err(TimelineError::NothingToEdit);
        }
        let repoint = |clip: &mut Arc<Clip>| {
            let kept = |source: SourceId| merged.get(&source).copied();
            let unrendered = clip.unrendered.and_then(kept);
            if kept(clip.source).is_some() || unrendered.is_some() {
                let clip = Arc::make_mut(clip);
                clip.source = kept(clip.source).unwrap_or(clip.source);
                clip.unrendered = unrendered.or(clip.unrendered);
            }
        };
        for (_, track) in sequence.tracks_mut() {
            track.clips.iter_mut().for_each(repoint);
        }
        let media = Arc::make_mut(&mut sequence.media);
        for (&duplicate, &kept) in &merged {
            let proxy = media
                .iter()
                .find(|item| item.id == duplicate)
                .and_then(|item| item.proxy.clone());
            if let Some(item) = media.iter_mut().find(|item| item.id == kept) {
                item.proxy = item.proxy.take().or(proxy);
            }
        }
        media.retain(|item| !merged.contains_key(&item.id));
        for nest in media.iter_mut().filter_map(|item| item.nest.as_mut()) {
            for track in nest.video.iter_mut().chain(&mut nest.audio) {
                track.clips.iter_mut().for_each(repoint);
            }
        }
        Ok(())
    }
}

fn used_media(sequence: &Sequence) -> HashSet<SourceId> {
    sequence
        .all_clips()
        .flat_map(|clip| [Some(clip.source), clip.unrendered])
        .flatten()
        .collect()
}

/// Each duplicate item and the earlier item it merges into.
fn duplicates(media: &[MediaItem]) -> HashMap<SourceId, SourceId> {
    let mut merged = HashMap::new();
    for (index, item) in media.iter().enumerate() {
        let original = media[..index]
            .iter()
            .find(|earlier| !merged.contains_key(&earlier.id) && same_footage(earlier, item));
        if let Some(original) = original {
            merged.insert(item.id, original.id);
        }
    }
    merged
}

/// Whether two items read the same file the same way, whatever they are called, where
/// they are filed, what is noted about them or which proxy they play.
fn same_footage(a: &MediaItem, b: &MediaItem) -> bool {
    if !a.is_file() || !b.is_file() || a.subclip.is_some() || b.subclip.is_some() {
        return false;
    }
    let as_b = MediaItem {
        id: b.id,
        name: b.name.clone(),
        bin: b.bin,
        log: b.log.clone(),
        proxy: b.proxy.clone(),
        placed: b.placed,
        frame_fit: b.frame_fit,
        ..a.clone()
    };
    as_b == *b
}
