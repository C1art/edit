use std::path::PathBuf;

use crate::edit::ops;
use crate::{
    Edit, MediaItem, Sequence, SourceId, Ticks, TimelineError, Track, TrackKind, TrackRef,
    VideoFormat,
};

/// Adds a track above the existing ones of its kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddTrack {
    pub kind: TrackKind,
}

impl Edit for AddTrack {
    fn name(&self) -> &'static str {
        "Add Track"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        sequence.tracks_of_mut(self.kind).push(Track::default());
        Ok(())
    }
}

/// Deletes a track with its clips; the tracks above it move down one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveTrack {
    pub track: TrackRef,
}

impl Edit for RemoveTrack {
    fn name(&self) -> &'static str {
        "Delete Track"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let tracks = sequence.tracks_of_mut(self.track.kind);
        match tracks.get(self.track.index) {
            Some(track) if track.locked => Err(TimelineError::TrackLocked(self.track)),
            Some(_) => {
                tracks.remove(self.track.index);
                Ok(())
            }
            None => Err(TimelineError::TrackNotFound(self.track)),
        }
    }
}

/// Names a track, or clears its name when `name` is none or blank.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameTrack {
    pub track: TrackRef,
    pub name: Option<String>,
}

impl Edit for RenameTrack {
    fn name(&self) -> &'static str {
        "Rename Track"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let name = self
            .name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_owned);
        let track = ops::track_mut(sequence, self.track)?;
        if track.name == name {
            return Err(TimelineError::NothingToEdit);
        }
        track.name = name;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackSwitch {
    Mute,
    Lock,
    Solo,
}

/// Turns a track's mute, lock or solo on or off.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetTrackSwitch {
    pub track: TrackRef,
    pub switch: TrackSwitch,
    pub on: bool,
}

impl Edit for SetTrackSwitch {
    fn name(&self) -> &'static str {
        match self.switch {
            TrackSwitch::Mute => "Toggle Track Output",
            TrackSwitch::Lock => "Toggle Track Lock",
            TrackSwitch::Solo => "Toggle Track Solo",
        }
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        if self.switch == TrackSwitch::Solo && self.track.kind != TrackKind::Audio {
            return Err(TimelineError::NothingToEdit);
        }
        let track = ops::track_mut(sequence, self.track)?;
        let value = match self.switch {
            TrackSwitch::Mute => &mut track.muted,
            TrackSwitch::Lock => &mut track.locked,
            TrackSwitch::Solo => &mut track.solo,
        };
        if *value == self.on {
            return Err(TimelineError::NothingToEdit);
        }
        *value = self.on;
        Ok(())
    }
}

/// Adds a probed file to the media pool under a new id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportMedia {
    pub name: String,
    pub path: PathBuf,
    pub duration: Ticks,
    pub video: Option<VideoFormat>,
    pub has_audio: bool,
}

impl Edit for ImportMedia {
    fn name(&self) -> &'static str {
        "Import"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        if self.duration <= Ticks::ZERO {
            return Err(TimelineError::NonPositiveDuration);
        }
        let id = SourceId(sequence.ids.next());
        sequence.media.push(MediaItem {
            id,
            name: self.name.clone(),
            path: self.path.clone(),
            duration: self.duration,
            video: self.video,
            has_audio: self.has_audio,
            proxy: None,
            bin: None,
            generator: None,
            nest: None,
            subclip: None,
            interpreted_rate: None,
        });
        Ok(())
    }
}

/// Removes an item from the media pool; refused while clips use it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveMedia {
    pub source: SourceId,
}

impl Edit for RemoveMedia {
    fn name(&self) -> &'static str {
        "Clear Media"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let in_use = sequence.all_clips().any(|clip| clip.source == self.source);
        if in_use {
            return Err(TimelineError::MediaInUse(self.source));
        }
        let count = sequence.media.len();
        sequence.media.retain(|item| item.id != self.source);
        if sequence.media.len() == count {
            return Err(TimelineError::MediaNotFound(self.source));
        }
        Ok(())
    }
}

/// Attaches a proxy to a media item, or detaches it with `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetProxy {
    pub source: SourceId,
    pub proxy: Option<PathBuf>,
}

impl Edit for SetProxy {
    fn name(&self) -> &'static str {
        "Attach Proxy"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let item = sequence
            .media
            .iter_mut()
            .find(|item| item.id == self.source)
            .ok_or(TimelineError::MediaNotFound(self.source))?;
        if item.proxy == self.proxy {
            return Err(TimelineError::NothingToEdit);
        }
        item.proxy.clone_from(&self.proxy);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetSequenceFormat {
    pub format: VideoFormat,
}

impl Edit for SetSequenceFormat {
    fn name(&self) -> &'static str {
        "Sequence Settings"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        if self.format.width == 0 || self.format.height == 0 || self.format == sequence.format {
            return Err(TimelineError::NothingToEdit);
        }
        sequence.format = self.format;
        Ok(())
    }
}
