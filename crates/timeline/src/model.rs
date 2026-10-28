use serde::{Deserialize, Serialize};

use crate::{FrameRate, Gain, Look, Marker, MediaItem, Pan, SourceId, Ticks, Timebase, Transition};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ClipId(pub u64);

/// Clips sharing a link move, trim and delete together, like the video and audio of
/// one source placed side by side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LinkId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TrackKind {
    Video,
    Audio,
}

/// Video tracks are numbered from the bottom layer up (V1 is under V2); audio tracks
/// from the top down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TrackRef {
    pub kind: TrackKind,
    pub index: usize,
}

impl TrackRef {
    pub const fn video(index: usize) -> Self {
        Self {
            kind: TrackKind::Video,
            index,
        }
    }

    pub const fn audio(index: usize) -> Self {
        Self {
            kind: TrackKind::Audio,
            index,
        }
    }
}

impl std::fmt::Display for TrackRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let letter = match self.kind {
            TrackKind::Video => 'V',
            TrackKind::Audio => 'A',
        };
        write!(f, "{letter}{}", self.index + 1)
    }
}

/// All times are in the owning sequence's timebase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clip {
    pub id: ClipId,
    pub source: SourceId,
    pub start: Ticks,
    pub duration: Ticks,
    pub source_in: Ticks,
    #[serde(default)]
    pub link: Option<LinkId>,
    /// Level of an audio clip; ignored for video.
    #[serde(default)]
    pub gain: Gain,
    #[serde(default)]
    pub look: Look,
    #[serde(default)]
    pub transition_in: Option<Transition>,
    /// Only used when no clip follows directly; a cut is owned by the incoming clip.
    #[serde(default)]
    pub transition_out: Option<Transition>,
}

impl Clip {
    /// An unlinked clip at unity gain with the default look and no transitions.
    pub fn new(
        id: ClipId,
        source: SourceId,
        start: Ticks,
        duration: Ticks,
        source_in: Ticks,
    ) -> Self {
        Self {
            id,
            source,
            start,
            duration,
            source_in,
            link: None,
            gain: Gain::UNITY,
            look: Look::default(),
            transition_in: None,
            transition_out: None,
        }
    }

    pub fn end(&self) -> Ticks {
        self.start + self.duration
    }

    pub fn source_out(&self) -> Ticks {
        self.source_in + self.duration
    }

    pub fn overlaps(&self, other: &Clip) -> bool {
        self.start < other.end() && other.start < self.end()
    }

    /// Whether `position` falls on this clip, counting its first tick but not its end.
    pub fn covers(&self, position: Ticks) -> bool {
        self.start <= position && position < self.end()
    }

    /// The source time playing at sequence `position`.
    pub fn source_time(&self, position: Ticks) -> Ticks {
        position - self.start + self.source_in
    }
}

/// Clips are kept sorted by `start` and never overlap.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    pub clips: Vec<Clip>,
    /// Hidden for video, silent for audio.
    #[serde(default)]
    pub muted: bool,
    /// Edits that would change a locked track's clips are rejected.
    #[serde(default)]
    pub locked: bool,
    /// Fader level of an audio track; ignored for video.
    #[serde(default)]
    pub volume: Gain,
    #[serde(default)]
    pub pan: Pan,
}

impl Track {
    pub fn clip_at(&self, position: Ticks) -> Option<&Clip> {
        let index = self.clips.partition_point(|clip| clip.start <= position);
        self.clips[..index]
            .last()
            .filter(|clip| clip.covers(position))
    }

    pub fn end(&self) -> Ticks {
        self.clips.last().map_or(Ticks::ZERO, Clip::end)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VideoFormat {
    pub width: u32,
    pub height: u32,
    pub frame_rate: FrameRate,
}

impl Default for VideoFormat {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            frame_rate: FrameRate::FPS_25,
        }
    }
}

/// Hands out ids that are never reused within a sequence's history.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdGenerator {
    next: u64,
}

impl IdGenerator {
    pub(crate) fn next(&mut self) -> u64 {
        self.next += 1;
        self.next
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sequence {
    pub timebase: Timebase,
    #[serde(default)]
    pub format: VideoFormat,
    #[serde(default)]
    pub media: Vec<MediaItem>,
    #[serde(default)]
    pub video: Vec<Track>,
    #[serde(default)]
    pub audio: Vec<Track>,
    #[serde(default)]
    pub master: Gain,
    /// Sorted by time.
    #[serde(default)]
    pub markers: Vec<Marker>,
    #[serde(default)]
    pub(crate) ids: IdGenerator,
}

impl Sequence {
    pub fn new(timebase: Timebase, format: VideoFormat) -> Self {
        Self {
            timebase,
            format,
            media: Vec::new(),
            video: Vec::new(),
            audio: Vec::new(),
            master: Gain::UNITY,
            markers: Vec::new(),
            ids: IdGenerator::default(),
        }
    }

    /// A sequence with the usual starting layout of three video and three audio tracks.
    pub fn with_default_tracks(timebase: Timebase, format: VideoFormat) -> Self {
        let mut sequence = Self::new(timebase, format);
        sequence.video.resize_with(3, Track::default);
        sequence.audio.resize_with(3, Track::default);
        sequence
    }

    pub fn tracks_of(&self, kind: TrackKind) -> &[Track] {
        match kind {
            TrackKind::Video => &self.video,
            TrackKind::Audio => &self.audio,
        }
    }

    pub(crate) fn tracks_of_mut(&mut self, kind: TrackKind) -> &mut Vec<Track> {
        match kind {
            TrackKind::Video => &mut self.video,
            TrackKind::Audio => &mut self.audio,
        }
    }

    pub fn track(&self, track: TrackRef) -> Option<&Track> {
        self.tracks_of(track.kind).get(track.index)
    }

    pub(crate) fn track_mut(&mut self, track: TrackRef) -> Option<&mut Track> {
        self.tracks_of_mut(track.kind).get_mut(track.index)
    }

    pub fn tracks(&self) -> impl Iterator<Item = (TrackRef, &Track)> {
        let video = self.video.iter().enumerate();
        let audio = self.audio.iter().enumerate();
        video
            .map(|(index, track)| (TrackRef::video(index), track))
            .chain(audio.map(|(index, track)| (TrackRef::audio(index), track)))
    }

    pub(crate) fn tracks_mut(&mut self) -> impl Iterator<Item = (TrackRef, &mut Track)> {
        let video = self.video.iter_mut().enumerate();
        let audio = self.audio.iter_mut().enumerate();
        video
            .map(|(index, track)| (TrackRef::video(index), track))
            .chain(audio.map(|(index, track)| (TrackRef::audio(index), track)))
    }

    pub fn clip(&self, id: ClipId) -> Option<(TrackRef, &Clip)> {
        self.tracks().find_map(|(track_ref, track)| {
            let clip = track.clips.iter().find(|clip| clip.id == id)?;
            Some((track_ref, clip))
        })
    }

    pub fn media_item(&self, id: SourceId) -> Option<&MediaItem> {
        self.media.iter().find(|item| item.id == id)
    }

    /// `ids` plus every clip linked to one of them, without duplicates.
    pub fn with_links(&self, ids: &[ClipId]) -> Vec<ClipId> {
        let links: Vec<LinkId> = ids.iter().filter_map(|&id| self.clip(id)?.1.link).collect();
        let mut all: Vec<ClipId> = self
            .tracks()
            .flat_map(|(_, track)| &track.clips)
            .filter(|clip| ids.contains(&clip.id) || clip.link.is_some_and(|l| links.contains(&l)))
            .map(|clip| clip.id)
            .collect();
        all.sort_unstable();
        all
    }

    /// This sequence with each media item's proxy in place of its file, for decoding
    /// pictures only, since proxies carry no sound.
    pub fn with_proxies(&self) -> Self {
        let mut sequence = self.clone();
        for item in &mut sequence.media {
            if let Some(proxy) = item.proxy.take() {
                item.path = proxy;
            }
        }
        sequence
    }

    /// The end of the last clip on any track.
    pub fn duration(&self) -> Ticks {
        self.tracks()
            .map(|(_, track)| track.end())
            .max()
            .unwrap_or(Ticks::ZERO)
    }

    /// The first marker strictly after `position`.
    pub fn marker_after(&self, position: Ticks) -> Option<&Marker> {
        self.markers.iter().find(|marker| marker.at > position)
    }

    /// The last marker strictly before `position`.
    pub fn marker_before(&self, position: Ticks) -> Option<&Marker> {
        self.markers
            .iter()
            .rev()
            .find(|marker| marker.at < position)
    }

    /// Clip starts and ends on all tracks, sorted and deduplicated, for snapping and
    /// jumping between edits.
    pub fn edit_points(&self) -> Vec<Ticks> {
        let mut points: Vec<Ticks> = self
            .tracks()
            .flat_map(|(_, track)| &track.clips)
            .flat_map(|clip| [clip.start, clip.end()])
            .collect();
        points.sort_unstable();
        points.dedup();
        points
    }
}
