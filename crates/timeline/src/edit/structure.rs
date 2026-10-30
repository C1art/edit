use std::path::PathBuf;
use std::sync::Arc;

use crate::edit::ops;
use crate::{
    Edit, FrameFit, FrameRate, MediaItem, Sequence, SourceId, Ticks, TimeDisplay, TimelineError,
    Track, TrackKind, TrackRef, VideoFormat,
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
    SyncLock,
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
            TrackSwitch::SyncLock => "Toggle Sync Lock",
        }
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        if self.switch == TrackSwitch::Solo && self.track.kind != TrackKind::Audio {
            return Err(TimelineError::NothingToEdit);
        }
        let track = ops::track_mut(sequence, self.track)?;
        // The track keeps the exception, sync lock being on by default.
        let (value, on) = match self.switch {
            TrackSwitch::Mute => (&mut track.muted, self.on),
            TrackSwitch::Lock => (&mut track.locked, self.on),
            TrackSwitch::Solo => (&mut track.solo, self.on),
            TrackSwitch::SyncLock => (&mut track.sync_unlocked, !self.on),
        };
        if *value == on {
            return Err(TimelineError::NothingToEdit);
        }
        *value = on;
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
    pub still: bool,
    /// The frame rate to play the file at instead of its own, as Interpret Footage sets.
    pub interpreted_rate: Option<FrameRate>,
    /// How long an edit places of a still when nothing is marked, instead of the standard.
    pub placed: Option<Ticks>,
    /// How clips of the pictures are scaled to the sequence frame when placed.
    pub frame_fit: Option<FrameFit>,
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
        Arc::make_mut(&mut sequence.media).push(MediaItem {
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
            interpreted_rate: self.interpreted_rate,
            interpreted_aspect: None,
            interpreted_color: None,
            interpreted_fields: None,
            alpha: Default::default(),
            log: Default::default(),
            offline: false,
            still: self.still,
            channel_map: None,
            placed: self.placed,
            frame_fit: self.frame_fit,
        });
        Ok(())
    }
}

/// Removes an item from the media pool and every clip of it, in nested sequences too.
/// Refused while clips rendered and replaced from it keep it to restore their original.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveMedia {
    pub source: SourceId,
}

impl Edit for RemoveMedia {
    fn name(&self) -> &'static str {
        "Remove from Project"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let source = self.source;
        if sequence.media_item(source).is_none() {
            return Err(TimelineError::MediaNotFound(source));
        }
        if sequence
            .all_clips()
            .any(|clip| clip.unrendered == Some(source))
        {
            return Err(TimelineError::MediaInUse(source));
        }
        for (_, track) in sequence.tracks_mut() {
            track.clips.retain(|clip| clip.source != source);
        }
        let mut emptied = Vec::new();
        for item in Arc::make_mut(&mut sequence.media) {
            let Some(nest) = &mut item.nest else {
                continue;
            };
            for track in nest.video.iter_mut().chain(&mut nest.audio) {
                let count = track.clips.len();
                track.clips.retain(|clip| clip.source != source);
                if track.clips.len() != count && !emptied.contains(&item.id) {
                    emptied.push(item.id);
                }
            }
        }
        Arc::make_mut(&mut sequence.media).retain(|item| item.id != source);
        for nest in emptied {
            sequence.fit_nest(nest);
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
        let item = Arc::make_mut(&mut sequence.media)
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

/// Changes how the sequence shows times.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetTimeDisplay {
    pub display: TimeDisplay,
}

impl Edit for SetTimeDisplay {
    fn name(&self) -> &'static str {
        "Display Format"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        if sequence.time_display == self.display {
            return Err(TimelineError::NothingToEdit);
        }
        sequence.time_display = self.display;
        Ok(())
    }
}

/// Sets whether the sequence's markers move with insert and ripple edits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetRippleMarkers {
    pub ripple: bool,
}

impl Edit for SetRippleMarkers {
    fn name(&self) -> &'static str {
        "Ripple Sequence Markers"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        if sequence.ripple_markers == self.ripple {
            return Err(TimelineError::NothingToEdit);
        }
        sequence.ripple_markers = self.ripple;
        Ok(())
    }
}
