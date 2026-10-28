use crate::{Clip, EditCommand, Sequence, Ticks, TimelineError, TrackIndex};

#[derive(Debug)]
pub struct AddClip {
    track: TrackIndex,
    clip: Clip,
}

impl AddClip {
    pub fn new(track: TrackIndex, clip: Clip) -> Self {
        Self { track, clip }
    }
}

impl EditCommand for AddClip {
    fn apply(&mut self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        if self.clip.duration <= Ticks::ZERO {
            return Err(TimelineError::NonPositiveDuration);
        }
        let track = sequence
            .tracks
            .get_mut(self.track.0)
            .ok_or(TimelineError::TrackNotFound(self.track))?;
        if let Some(existing) = track.clips.iter().find(|c| c.overlaps(&self.clip)) {
            return Err(TimelineError::ClipOverlap {
                track: self.track,
                at: existing.start.max(self.clip.start),
            });
        }
        let position = track.clips.partition_point(|c| c.start < self.clip.start);
        track.clips.insert(position, self.clip.clone());
        Ok(())
    }

    fn revert(&mut self, sequence: &mut Sequence) {
        if let Some(track) = sequence.tracks.get_mut(self.track.0) {
            track.clips.retain(|c| c.id != self.clip.id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClipId, SourceId, Timebase, Timeline, Track, TrackKind};

    fn timeline_with_one_track() -> Timeline {
        let mut sequence = Sequence::new(Timebase::new(1, 90000).unwrap());
        sequence.tracks.push(Track {
            kind: TrackKind::Video,
            clips: Vec::new(),
        });
        Timeline::new(sequence)
    }

    fn clip(id: u64, start: i64, duration: i64) -> Clip {
        Clip {
            id: ClipId(id),
            source: SourceId(0),
            start: Ticks(start),
            duration: Ticks(duration),
            source_in: Ticks::ZERO,
        }
    }

    fn add(clip: Clip) -> Box<dyn EditCommand> {
        Box::new(AddClip::new(TrackIndex(0), clip))
    }

    fn clip_ids(timeline: &Timeline) -> Vec<u64> {
        timeline.sequence().tracks[0]
            .clips
            .iter()
            .map(|c| c.id.0)
            .collect()
    }

    #[test]
    fn inserts_clips_sorted_by_start() {
        let mut timeline = timeline_with_one_track();
        timeline.execute(add(clip(1, 200, 100))).unwrap();
        timeline.execute(add(clip(2, 0, 100))).unwrap();
        assert_eq!(clip_ids(&timeline), [2, 1]);
    }

    #[test]
    fn rejects_overlap_without_mutating_or_recording() {
        let mut timeline = timeline_with_one_track();
        timeline.execute(add(clip(1, 0, 100))).unwrap();
        let error = timeline.execute(add(clip(2, 99, 10))).unwrap_err();
        assert!(matches!(
            error,
            TimelineError::ClipOverlap { at: Ticks(99), .. }
        ));
        assert_eq!(clip_ids(&timeline), [1]);
        assert!(timeline.undo());
        assert!(!timeline.undo());
    }

    #[test]
    fn undo_then_redo_restores_each_state() {
        let mut timeline = timeline_with_one_track();
        let empty = timeline.sequence().clone();
        timeline.execute(add(clip(1, 0, 100))).unwrap();
        let with_clip = timeline.sequence().clone();

        assert!(timeline.undo());
        assert_eq!(timeline.sequence(), &empty);
        assert!(timeline.redo().unwrap());
        assert_eq!(timeline.sequence(), &with_clip);
    }

    #[test]
    fn new_edit_clears_redo_stack() {
        let mut timeline = timeline_with_one_track();
        timeline.execute(add(clip(1, 0, 100))).unwrap();
        timeline.undo();
        timeline.execute(add(clip(2, 500, 100))).unwrap();
        assert!(!timeline.redo().unwrap());
    }
}
