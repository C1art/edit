use crate::edit::ops::{self, Relinks};
use crate::{Clip, ClipId, Edit, Gain, LinkId, Sequence, SourceId, Ticks, TimelineError, TrackRef};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceMode {
    /// Pushes everything at and after the insertion point later on every unlocked track.
    Insert,
    /// Replaces whatever is in the way on the target tracks.
    Overwrite,
}

/// Puts a range of a media item into the sequence, linking its video and audio clips.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceClip {
    pub source: SourceId,
    pub source_in: Ticks,
    pub duration: Ticks,
    pub at: Ticks,
    /// Video tracks are ignored for media without video, and audio tracks likewise.
    pub video_track: Option<usize>,
    pub audio_track: Option<usize>,
    pub mode: PlaceMode,
}

impl Edit for PlaceClip {
    fn name(&self) -> &'static str {
        match self.mode {
            PlaceMode::Insert => "Insert",
            PlaceMode::Overwrite => "Overwrite",
        }
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let media = sequence
            .media_item(self.source)
            .ok_or(TimelineError::MediaNotFound(self.source))?;
        let video = self.video_track.filter(|_| media.video.is_some());
        let audio = self.audio_track.filter(|_| media.has_audio);
        let targets: Vec<TrackRef> = video
            .map(TrackRef::video)
            .into_iter()
            .chain(audio.map(TrackRef::audio))
            .collect();
        if targets.is_empty() {
            return Err(TimelineError::NothingToEdit);
        }
        if self.duration <= Ticks::ZERO {
            return Err(TimelineError::NonPositiveDuration);
        }
        let mut relinks = Relinks::default();
        match self.mode {
            PlaceMode::Insert => {
                let tracks = ops::unlocked_tracks(sequence);
                ops::split(sequence, &tracks, self.at, &mut relinks);
                ops::shift(sequence, &tracks, self.at, self.duration, &[]);
            }
            PlaceMode::Overwrite => {
                for &track in &targets {
                    ops::clear(
                        sequence,
                        track,
                        self.at..self.at + self.duration,
                        &mut relinks,
                    )?;
                }
            }
        }
        let link = (targets.len() > 1).then(|| LinkId(sequence.ids.next()));
        for track in targets {
            let clip = Clip {
                id: ClipId(sequence.ids.next()),
                source: self.source,
                start: self.at,
                duration: self.duration,
                source_in: self.source_in,
                link,
                gain: Gain::UNITY,
            };
            ops::insert(sequence, track, clip)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RazorTarget {
    Tracks(Vec<TrackRef>),
    /// These clips and the clips linked to them.
    Clips(Vec<ClipId>),
}

/// Cuts clips in two at `at`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Razor {
    pub at: Ticks,
    pub target: RazorTarget,
}

impl Edit for Razor {
    fn name(&self) -> &'static str {
        "Razor"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let tracks: Vec<TrackRef> = match &self.target {
            RazorTarget::Tracks(tracks) => tracks.clone(),
            RazorTarget::Clips(clips) => sequence
                .with_links(clips)
                .into_iter()
                .filter_map(|id| sequence.clip(id))
                .filter(|(_, clip)| clip.covers(self.at))
                .map(|(track, _)| track)
                .collect(),
        };
        match ops::split(sequence, &tracks, self.at, &mut Relinks::default()) {
            0 => Err(TimelineError::NothingToEdit),
            _ => Ok(()),
        }
    }
}
