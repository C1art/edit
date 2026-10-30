use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;
use std::sync::Arc;

use crate::model::IdGenerator;
use crate::{
    ChannelMap, ClipId, Edit, GroupId, ImportMedia, LinkId, MediaItem, MediaLog, Nest, Sequence,
    SourceId, Ticks, TimelineError,
};

fn media_item(sequence: &mut Sequence, source: SourceId) -> Result<&mut MediaItem, TimelineError> {
    Arc::make_mut(&mut sequence.media)
        .iter_mut()
        .find(|item| item.id == source)
        .ok_or(TimelineError::MediaNotFound(source))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameMedia {
    pub source: SourceId,
    pub name: String,
}

impl Edit for RenameMedia {
    fn name(&self) -> &'static str {
        "Rename"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let name = self.name.trim();
        let item = media_item(sequence, self.source)?;
        if name.is_empty() || item.name == name {
            return Err(TimelineError::NothingToEdit);
        }
        name.clone_into(&mut item.name);
        Ok(())
    }
}

/// Plays the file's channels `map` names on each side, or all of them mixed to stereo
/// again for `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetChannelMap {
    pub source: SourceId,
    pub map: Option<ChannelMap>,
}

impl Edit for SetChannelMap {
    fn name(&self) -> &'static str {
        "Modify Audio Channels"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let item = media_item(sequence, self.source)?;
        if !item.has_audio || !item.is_file() || item.channel_map == self.map {
            return Err(TimelineError::NothingToEdit);
        }
        item.channel_map = self.map;
        Ok(())
    }
}

/// Replaces what is logged about a media item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetMediaLog {
    pub source: SourceId,
    pub log: MediaLog,
}

impl Edit for SetMediaLog {
    fn name(&self) -> &'static str {
        "Edit Metadata"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let item = media_item(sequence, self.source)?;
        if item.log == self.log {
            return Err(TimelineError::NothingToEdit);
        }
        item.log.clone_from(&self.log);
        Ok(())
    }
}

/// Modify > Timecode: where a file's timecode starts, in frames at the rate it plays, or
/// `None` to count from zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetMediaTimecode {
    pub source: SourceId,
    pub timecode: Option<i64>,
}

impl Edit for SetMediaTimecode {
    fn name(&self) -> &'static str {
        "Modify Timecode"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let item = media_item(sequence, self.source)?;
        let timecode = self.timecode.filter(|&frame| frame > 0);
        if !item.is_file() || item.video.is_none() || item.timecode == timecode {
            return Err(TimelineError::NothingToEdit);
        }
        item.timecode = timecode;
        Ok(())
    }
}

/// Points a media item at another file, as when the original moved or a better render
/// replaces it. Clips keep their source times, so the file must be long enough for them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelinkMedia {
    pub source: SourceId,
    pub media: ImportMedia,
}

impl Edit for RelinkMedia {
    fn name(&self) -> &'static str {
        "Replace Footage"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let item = media_item(sequence, self.source)?;
        if !item.is_file() || (item.path == self.media.path && !item.offline) {
            return Err(TimelineError::NothingToEdit);
        }
        // A name the user chose survives; one that only echoed the old file follows the new.
        if is_file_name(&item.name, &item.path) {
            self.media.name.clone_into(&mut item.name);
        }
        item.path.clone_from(&self.media.path);
        item.duration = self.media.duration;
        item.video = self.media.video;
        item.has_audio = self.media.has_audio;
        item.still = self.media.still;
        item.proxy = None;
        item.offline = false;
        Ok(())
    }
}

/// Stops the project reading the files behind media items, as Premiere's Make Offline
/// does before the files are moved or replaced; clips keep their places and show as
/// offline until the items are relinked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MakeOffline {
    pub sources: Vec<SourceId>,
}

impl Edit for MakeOffline {
    fn name(&self) -> &'static str {
        "Make Offline"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let mut changed = false;
        for &source in &self.sources {
            let item = media_item(sequence, source)?;
            if item.is_file() && !item.offline {
                item.offline = true;
                changed = true;
            }
        }
        if changed {
            Ok(())
        } else {
            Err(TimelineError::NothingToEdit)
        }
    }
}

fn is_file_name(name: &str, path: &Path) -> bool {
    path.file_name().is_some_and(|file| *file == *name)
}

/// Adds an item to the pool standing for part of a file, in the same bin as the item
/// it was marked in, which may itself be a subclip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MakeSubclip {
    pub source: SourceId,
    /// In the file's time.
    pub range: Range<Ticks>,
    pub name: String,
}

impl Edit for MakeSubclip {
    fn name(&self) -> &'static str {
        "Make Subclip"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let item = sequence
            .media_item(self.source)
            .ok_or(TimelineError::MediaNotFound(self.source))?
            .clone();
        if !item.is_file() {
            return Err(TimelineError::NothingToEdit);
        }
        let range = self.range.start.max(Ticks::ZERO)..self.range.end.min(item.duration);
        if range.is_empty() {
            return Err(TimelineError::NonPositiveDuration);
        }
        let subclip = MediaItem {
            id: SourceId(sequence.ids.next()),
            name: self.name.clone(),
            subclip: Some(range),
            ..item
        };
        Arc::make_mut(&mut sequence.media).push(subclip);
        Ok(())
    }
}

/// Changes the part of the file a subclip stands for, or with no range makes it stand
/// for the whole file again, as a master clip. Its clips keep their source times.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditSubclip {
    pub source: SourceId,
    /// In the file's time.
    pub range: Option<Range<Ticks>>,
}

impl Edit for EditSubclip {
    fn name(&self) -> &'static str {
        "Edit Subclip"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let item = media_item(sequence, self.source)?;
        let range = self
            .range
            .as_ref()
            .map(|range| range.start.max(Ticks::ZERO)..range.end.min(item.duration));
        if range.as_ref().is_some_and(Range::is_empty) {
            return Err(TimelineError::NonPositiveDuration);
        }
        if item.subclip.is_none() || item.subclip == range {
            return Err(TimelineError::NothingToEdit);
        }
        item.subclip = range;
        Ok(())
    }
}

/// Copies a media item, as the Project panel's Duplicate does: a sequence's copy is
/// edited apart from the original, and clips placed from the original keep using it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateMedia {
    pub source: SourceId,
}

impl Edit for DuplicateMedia {
    fn name(&self) -> &'static str {
        "Duplicate"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let index = sequence
            .media
            .iter()
            .position(|item| item.id == self.source)
            .ok_or(TimelineError::MediaNotFound(self.source))?;
        let mut copy = sequence.media[index].clone();
        copy.id = SourceId(sequence.ids.next());
        copy.name.push_str(" Copy");
        if let Some(nest) = &mut copy.nest {
            renumber(nest, &mut sequence.ids);
        }
        Arc::make_mut(&mut sequence.media).insert(index + 1, copy);
        Ok(())
    }
}

/// Gives a copied sequence's clips, links and groups ids of their own, so nothing kept per
/// clip, such as a selection or a cached preview, mistakes the copy for its original.
pub(super) fn renumber(nest: &mut Nest, ids: &mut IdGenerator) {
    let mut links: HashMap<LinkId, LinkId> = HashMap::new();
    let mut groups: HashMap<GroupId, GroupId> = HashMap::new();
    let tracks = nest.video.iter_mut().chain(&mut nest.audio);
    for clip in tracks.flat_map(|track| track.clips.iter_mut().map(Arc::make_mut)) {
        clip.id = ClipId(ids.next());
        if let Some(link) = &mut clip.link {
            *link = *links.entry(*link).or_insert_with(|| LinkId(ids.next()));
        }
        if let Some(group) = &mut clip.group {
            *group = *groups.entry(*group).or_insert_with(|| GroupId(ids.next()));
        }
    }
}

/// Adds a mono item for each of a file's `channels` after it, as Premiere's Breakout to
/// Mono does so each microphone can be placed and mixed on its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreakoutToMono {
    pub source: SourceId,
    pub channels: u16,
}

impl Edit for BreakoutToMono {
    fn name(&self) -> &'static str {
        "Breakout to Mono"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let index = sequence
            .media
            .iter()
            .position(|item| item.id == self.source)
            .ok_or(TimelineError::MediaNotFound(self.source))?;
        let original = &sequence.media[index];
        if !original.has_audio || !original.is_file() || self.channels < 2 {
            return Err(TimelineError::NothingToEdit);
        }
        let mono: Vec<_> = (0..self.channels)
            .map(|channel| MediaItem {
                id: SourceId(sequence.ids.next()),
                name: format!("{} {}", original.name, channel_name(channel, self.channels)),
                video: None,
                proxy: None,
                frame_fit: None,
                channel_map: Some(ChannelMap {
                    left: channel,
                    right: channel,
                }),
                ..original.clone()
            })
            .collect();
        Arc::make_mut(&mut sequence.media).splice(index + 1..index + 1, mono);
        Ok(())
    }
}

fn channel_name(channel: u16, channels: u16) -> String {
    match (channel, channels) {
        (0, 2) => "Left".to_owned(),
        (1, 2) => "Right".to_owned(),
        _ => format!("Channel {}", channel + 1),
    }
}
