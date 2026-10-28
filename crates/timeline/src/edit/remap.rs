use crate::edit::ops;
use crate::{ClipId, Edit, Sequence, Speed, Ticks, TimeRemap, TimelineError, TrackKind};

/// Keyframes the speed of a video clip at sequence position `at`, starting time
/// remapping on it if needed. Linked sound is left at its own speed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddSpeedKeyframe {
    pub clip: ClipId,
    pub at: Ticks,
}

impl Edit for AddSpeedKeyframe {
    fn name(&self) -> &'static str {
        "Add Speed Keyframe"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let (track, clip) = sequence
            .clip(self.clip)
            .ok_or(TimelineError::ClipNotFound(self.clip))?;
        if track.kind != TrackKind::Video
            || clip.hold
            || clip.reverse
            || !clip.covers(self.at)
            || self.at == clip.start
        {
            return Err(TimelineError::NothingToEdit);
        }
        let clip = ops::clip_mut(sequence, self.clip)?;
        let offset = self.at - clip.start;
        if clip.remap.is_none() {
            clip.remap = Some(TimeRemap::constant(
                clip.source_in,
                clip.duration,
                clip.speed,
            ));
            clip.speed = Speed::NORMAL;
        }
        if !clip
            .remap
            .as_mut()
            .is_some_and(|remap| remap.insert(offset))
        {
            return Err(TimelineError::NothingToEdit);
        }
        Ok(())
    }
}

/// Removes the speed keyframe `offset` ticks into a clip, joining the segments either
/// side into one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveSpeedKeyframe {
    pub clip: ClipId,
    pub offset: Ticks,
}

impl Edit for RemoveSpeedKeyframe {
    fn name(&self) -> &'static str {
        "Remove Speed Keyframe"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        reshape(sequence, self.clip, |remap| remap.remove(self.offset))
    }
}

/// Plays the remapped segment starting at anchor `index` at `speed`. The clip keeps
/// its first and last frames, so it grows or shrinks, stopping at the next clip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetSegmentSpeed {
    pub clip: ClipId,
    pub index: usize,
    pub speed: Speed,
}

impl Edit for SetSegmentSpeed {
    fn name(&self) -> &'static str {
        "Time Remapping Speed"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        if !self.speed.is_in_range() {
            return Err(TimelineError::SpeedOutOfRange(self.clip));
        }
        reshape(sequence, self.clip, |remap| {
            remap.set_speed(self.index, self.speed)
        })
    }
}

/// Plays a remapped clip's source range at one constant speed again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveTimeRemap {
    pub clip: ClipId,
}

impl Edit for RemoveTimeRemap {
    fn name(&self) -> &'static str {
        "Remove Time Remapping"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let clip = ops::clip_mut(sequence, self.clip)?;
        if clip.remap.is_none() {
            return Err(TimelineError::NothingToEdit);
        }
        let source = clip.source_out() - clip.source_in;
        clip.speed = Speed::stretch(source, clip.duration)
            .ok_or(TimelineError::SpeedOutOfRange(self.clip))?;
        clip.remap = None;
        Ok(())
    }
}

/// Changes a clip's speed curve with `change`, then fits the clip to it so it still
/// starts and ends on the same frames, without running into the clip after it.
fn reshape(
    sequence: &mut Sequence,
    id: ClipId,
    change: impl FnOnce(&mut TimeRemap) -> bool,
) -> Result<(), TimelineError> {
    let (track, clip) = sequence.clip(id).ok_or(TimelineError::ClipNotFound(id))?;
    let room = sequence
        .track(track)
        .and_then(|track| track.clips.iter().find(|next| next.start >= clip.end()))
        .map(|next| next.start - clip.start);
    let clip = ops::clip_mut(sequence, id)?;
    let Some(remap) = &mut clip.remap else {
        return Err(TimelineError::NothingToEdit);
    };
    let (first, last) = (remap.source_at(Ticks::ZERO), remap.source_at(clip.duration));
    if !change(remap) {
        return Err(TimelineError::NothingToEdit);
    }
    let head = remap.offset_of(first);
    remap.shift(Ticks::ZERO - head, Ticks::ZERO);
    let duration = remap.offset_of(last).max(Ticks(1));
    let source_in = remap.source_at(Ticks::ZERO);
    clip.duration = room.map_or(duration, |room| duration.min(room));
    clip.source_in = source_in;
    Ok(())
}
