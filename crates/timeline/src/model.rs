use serde::{Deserialize, Serialize};

use crate::{
    AudioEffect, Bin, Captions, FrameRate, Gain, Label, Look, Marker, Marks, MediaItem, Pan, Param,
    SourceId, Speed, Ticks, TimeRemap, Timebase, Transition,
};

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
    pub speed: Speed,
    /// Plays its source range backwards, from `source_out` down to `source_in`.
    #[serde(default)]
    pub reverse: bool,
    /// Off normal speed, stretches audio in time without changing its pitch.
    #[serde(default)]
    pub maintain_pitch: bool,
    /// Shows the frame at `source_in` for the whole clip, ignoring speed and direction.
    #[serde(default)]
    pub hold: bool,
    #[serde(default)]
    pub link: Option<LinkId>,
    /// Left out of playback and export but kept on the timeline.
    #[serde(default)]
    pub disabled: bool,
    /// Level of an audio clip; ignored for video.
    #[serde(default)]
    pub gain: Gain,
    /// Level of an audio clip in decibels on top of its gain, keyframed over source time.
    #[serde(default)]
    pub volume: Param,
    /// Effects on an audio clip's sound; ignored for video.
    #[serde(default)]
    pub audio_effects: Vec<AudioEffect>,
    #[serde(default)]
    pub look: Look,
    #[serde(default)]
    pub transition_in: Option<Transition>,
    /// Only used when no clip follows directly; a cut is owned by the incoming clip.
    #[serde(default)]
    pub transition_out: Option<Transition>,
    /// Shown instead of the track's color on the timeline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<Label>,
    /// Replaces `speed` and `reverse` on a video clip whose speed changes over time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remap: Option<TimeRemap>,
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
            speed: Speed::NORMAL,
            reverse: false,
            maintain_pitch: false,
            hold: false,
            link: None,
            disabled: false,
            gain: Gain::UNITY,
            volume: Param::default(),
            audio_effects: Vec::new(),
            look: Look::default(),
            transition_in: None,
            transition_out: None,
            label: None,
            remap: None,
        }
    }

    pub fn end(&self) -> Ticks {
        self.start + self.duration
    }

    pub fn source_out(&self) -> Ticks {
        if self.hold {
            return self.source_in + Ticks(1);
        }
        if let Some(remap) = &self.remap {
            return remap.source_at(self.duration);
        }
        self.source_in + self.speed.source_span(self.duration)
    }

    pub fn overlaps(&self, other: &Clip) -> bool {
        self.start < other.end() && other.start < self.end()
    }

    /// Whether `position` falls on this clip, counting its first tick but not its end.
    pub fn covers(&self, position: Ticks) -> bool {
        self.start <= position && position < self.end()
    }

    /// The source time playing at sequence `position`. A reversed clip counts back from
    /// its end, so `source_in` is what its last tick shows.
    pub fn source_time(&self, position: Ticks) -> Ticks {
        if self.hold {
            self.source_in
        } else if let Some(remap) = &self.remap {
            remap.source_at(position - self.start)
        } else if self.reverse {
            self.source_in + self.speed.source_span(self.end() - position) - Ticks(1)
        } else {
            self.source_in + self.speed.source_span(position - self.start)
        }
    }

    /// The sequence position where source time `source` plays.
    pub fn position_of(&self, source: Ticks) -> Ticks {
        if self.hold {
            self.start
        } else if let Some(remap) = &self.remap {
            self.start + remap.offset_of(source)
        } else if self.reverse {
            self.end() - self.speed.sequence_span(source + Ticks(1) - self.source_in)
        } else {
            self.start + self.speed.sequence_span(source - self.source_in)
        }
    }

    /// Moves the head `delta` later in the sequence (earlier when negative), keeping the
    /// tail and the source showing there. The caller moves `start`.
    pub(crate) fn trim_head(&mut self, delta: Ticks) {
        if let (Some(remap), false) = (&mut self.remap, self.hold) {
            remap.shift(Ticks::ZERO - delta, Ticks::ZERO);
            self.source_in = remap.source_at(Ticks::ZERO);
        } else if !self.reverse && !self.hold {
            self.source_in = self.source_in + self.speed.source_span(delta);
        }
        self.duration = self.duration - delta;
    }

    /// Moves the tail `delta` later in the sequence, keeping the head and the source
    /// showing there.
    pub(crate) fn trim_tail(&mut self, delta: Ticks) {
        if self.reverse && !self.hold && self.remap.is_none() {
            let source_out = self.source_out();
            self.duration = self.duration + delta;
            self.source_in = source_out - self.speed.source_span(self.duration);
        } else {
            self.duration = self.duration + delta;
        }
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
    /// While any audio track is soloed, only soloed ones are heard.
    #[serde(default)]
    pub solo: bool,
    /// Fader level of an audio track; ignored for video.
    #[serde(default)]
    pub volume: Gain,
    #[serde(default)]
    pub pan: Pan,
    /// An audio track's level in decibels after its fader, keyframed over sequence time
    /// rather than any clip's, so it stays put when clips move.
    #[serde(default)]
    pub automation: Param,
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
    pub bins: Vec<Bin>,
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
    pub marks: Marks,
    #[serde(default)]
    pub captions: Captions,
    #[serde(default)]
    pub(crate) ids: IdGenerator,
}

impl Sequence {
    pub fn new(timebase: Timebase, format: VideoFormat) -> Self {
        Self {
            timebase,
            format,
            media: Vec::new(),
            bins: Vec::new(),
            video: Vec::new(),
            audio: Vec::new(),
            master: Gain::UNITY,
            markers: Vec::new(),
            marks: Marks::default(),
            captions: Captions::default(),
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

    /// Whether an audio track reaches the mix, given its mute and every track's solo.
    pub fn is_heard(&self, track: &Track) -> bool {
        !track.muted && (track.solo || !self.audio.iter().any(|track| track.solo))
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
