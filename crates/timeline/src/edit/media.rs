use std::ops::Range;
use std::path::Path;

use crate::{Edit, ImportMedia, MediaItem, Sequence, SourceId, Ticks, TimelineError};

fn media_item(sequence: &mut Sequence, source: SourceId) -> Result<&mut MediaItem, TimelineError> {
    sequence
        .media
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
        if !item.is_file() || item.path == self.media.path {
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
        item.proxy = None;
        Ok(())
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
        sequence.media.push(subclip);
        Ok(())
    }
}
