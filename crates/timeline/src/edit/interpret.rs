use crate::{Clip, Edit, FrameRate, Sequence, SourceId, Speed, Ticks, TimelineError, Track};

/// Plays a file's pictures at `rate` instead of the rate it was recorded at, or at its
/// own again for `None`. Its clips keep the frames they show, so they grow or shrink,
/// stopping at the next clip on their track.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterpretFootage {
    pub source: SourceId,
    pub rate: Option<FrameRate>,
}

impl Edit for InterpretFootage {
    fn name(&self) -> &'static str {
        "Interpret Footage"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let item = sequence
            .media
            .iter_mut()
            .find(|item| item.id == self.source)
            .ok_or(TimelineError::MediaNotFound(self.source))?;
        let native = item.video.map(|video| video.frame_rate);
        let rate = self.rate.filter(|&rate| Some(rate) != native);
        if !item.is_file() || native.is_none() || item.interpreted_rate == rate {
            return Err(TimelineError::NothingToEdit);
        }
        item.interpreted_rate = rate;
        let conform = item.conform();
        let nests = sequence
            .media
            .iter_mut()
            .filter_map(|item| item.nest.as_mut())
            .flat_map(|nest| nest.video.iter_mut().chain(&mut nest.audio));
        let tracks = sequence.video.iter_mut().chain(&mut sequence.audio);
        for track in tracks.chain(nests) {
            conform_track(track, self.source, conform);
        }
        let nest_ids: Vec<SourceId> = sequence
            .media
            .iter()
            .filter(|item| item.nest.is_some())
            .map(|item| item.id)
            .collect();
        for id in nest_ids {
            sequence.fit_nest(id);
        }
        Ok(())
    }
}

fn conform_track(track: &mut Track, source: SourceId, conform: Speed) {
    let starts: Vec<Ticks> = track.clips.iter().map(|clip| clip.start).collect();
    for (index, clip) in track.clips.iter_mut().enumerate() {
        if clip.source != source {
            continue;
        }
        let next = starts.get(index + 1).copied();
        conform_clip(clip, conform, next);
    }
}

fn conform_clip(clip: &mut Clip, conform: Speed, next: Option<Ticks>) {
    let span = clip.source_out() - clip.source_in;
    let reversed_out = clip.source_out();
    clip.conform = conform;
    // Held frames and remapped speeds don't run at the media's rate.
    if clip.hold || clip.remap.is_some() {
        return;
    }
    let wanted = clip.rate().sequence_span(span).max(Ticks(1));
    clip.duration = next.map_or(wanted, |next| wanted.min(next - clip.start));
    if clip.reverse {
        clip.source_in = reversed_out - clip.rate().source_span(clip.duration);
    }
}
