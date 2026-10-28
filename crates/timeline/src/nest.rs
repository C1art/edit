use serde::{Deserialize, Serialize};

use crate::{Clip, Gain, Marker, Marks, Sequence, SourceId, Ticks, TimelineError, Track, TrackRef};

/// A sequence placed as media in the project's sequence. It has its own tracks, mix and
/// markers but shares the project's media pool, timebase and frame format.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Nest {
    #[serde(default)]
    pub video: Vec<Track>,
    #[serde(default)]
    pub audio: Vec<Track>,
    #[serde(default)]
    pub master: Gain,
    #[serde(default)]
    pub markers: Vec<Marker>,
    #[serde(default)]
    pub marks: Marks,
}

impl Nest {
    /// The usual starting layout of three video and three audio tracks.
    pub fn with_default_tracks() -> Self {
        let mut nest = Self::default();
        nest.video.resize_with(3, Track::default);
        nest.audio.resize_with(3, Track::default);
        nest
    }

    /// The end of the last clip on any track.
    pub fn content_end(&self) -> Ticks {
        self.video
            .iter()
            .chain(&self.audio)
            .map(Track::end)
            .max()
            .unwrap_or(Ticks::ZERO)
    }

    pub fn tracks(&self) -> impl Iterator<Item = (TrackRef, &Track)> {
        let video = self.video.iter().enumerate();
        let audio = self.audio.iter().enumerate();
        video
            .map(|(index, track)| (TrackRef::video(index), track))
            .chain(audio.map(|(index, track)| (TrackRef::audio(index), track)))
    }

    fn clips(&self) -> impl Iterator<Item = &Clip> {
        self.tracks().flat_map(|(_, track)| &track.clips)
    }
}

impl Sequence {
    /// Nest `id` shown as a sequence of its own, sharing this one's media.
    pub fn nest_view(&self, id: SourceId) -> Option<Sequence> {
        let nest = self.media_item(id)?.nest.as_ref()?;
        Some(Sequence {
            video: nest.video.clone(),
            audio: nest.audio.clone(),
            master: nest.master,
            markers: nest.markers.clone(),
            marks: nest.marks,
            ..self.clone()
        })
    }

    /// Trades this sequence's tracks, mix, markers and marks for those of nest `id`, so an edit
    /// of the sequence edits the nest. Swapping again trades them back.
    pub(crate) fn swap_nest(&mut self, id: SourceId) -> Result<(), TimelineError> {
        let nest = self
            .media
            .iter_mut()
            .find(|item| item.id == id)
            .and_then(|item| item.nest.as_mut())
            .ok_or(TimelineError::MediaNotFound(id))?;
        std::mem::swap(&mut self.video, &mut nest.video);
        std::mem::swap(&mut self.audio, &mut nest.audio);
        std::mem::swap(&mut self.master, &mut nest.master);
        std::mem::swap(&mut self.markers, &mut nest.markers);
        std::mem::swap(&mut self.marks, &mut nest.marks);
        Ok(())
    }

    /// Every clip on the sequence's own tracks and inside its nests.
    pub fn all_clips(&self) -> impl Iterator<Item = &Clip> {
        let nested = self
            .media
            .iter()
            .filter_map(|item| item.nest.as_ref())
            .flat_map(Nest::clips);
        self.tracks()
            .flat_map(|(_, track)| &track.clips)
            .chain(nested)
    }

    /// Makes nest `id` as long as its content, or as long as the clips using it need, so
    /// shortening a nest leaves its clips showing black rather than breaking them. Its
    /// pictures and sound follow whether it has video and audio tracks.
    pub(crate) fn fit_nest(&mut self, id: SourceId) {
        let used = self
            .all_clips()
            .filter(|clip| clip.source == id)
            .map(Clip::source_out)
            .max()
            .unwrap_or(Ticks::ZERO);
        let format = self.format;
        let Some(item) = self.media.iter_mut().find(|item| item.id == id) else {
            return;
        };
        if let Some(nest) = &item.nest {
            item.duration = nest.content_end().max(used).max(Ticks(1));
            item.video = (!nest.video.is_empty()).then_some(format);
            item.has_audio = !nest.audio.is_empty();
        }
    }

    /// Whether nest `id` shows itself, directly or through other nests.
    pub(crate) fn nest_contains_itself(&self, id: SourceId) -> bool {
        let mut pending = vec![id];
        let mut seen = Vec::new();
        while let Some(current) = pending.pop() {
            let Some(nest) = self.media_item(current).and_then(|item| item.nest.as_ref()) else {
                continue;
            };
            for source in nest.clips().map(|clip| clip.source) {
                if source == id {
                    return true;
                }
                if !seen.contains(&source) {
                    seen.push(source);
                    pending.push(source);
                }
            }
        }
        false
    }
}
