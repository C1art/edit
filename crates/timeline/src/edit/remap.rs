use crate::edit::ops;
use crate::{
    Clip, ClipId, Edit, Sequence, Speed, SpeedRamp, Ticks, TimeRemap, TimelineError, TrackKind,
};

/// Keyframes the speed of a video clip at sequence position `at`, starting time
/// remapping on it if needed. Linked sound playing in step with it follows its speed.
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
        let offset = self.at - clip.start;
        for id in in_step(sequence, self.clip) {
            let clip = ops::clip_mut(sequence, id)?;
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
        }
        Ok(())
    }
}

/// Gives video clips, and the sound in step with each, a ready-made speed ramp in place
/// of the speed they had, keeping their lengths and the source they play. Held,
/// reversed and too short clips keep theirs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplySpeedRamp {
    pub clips: Vec<ClipId>,
    pub ramp: SpeedRamp,
}

impl Edit for ApplySpeedRamp {
    fn name(&self) -> &'static str {
        "Speed Ramp"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let frames = (sequence.format.frame_rate, sequence.timebase);
        let mut ramped = Vec::new();
        for &id in &self.clips {
            let Some((track, clip)) = sequence.clip(id) else {
                continue;
            };
            if track.kind != TrackKind::Video || clip.hold || clip.reverse || ramped.contains(&id) {
                continue;
            }
            let source = clip.source_in..clip.source_out();
            let Some(remap) = self.ramp.remap(source, clip.duration, frames) else {
                continue;
            };
            for id in in_step(sequence, id) {
                let clip = ops::clip_mut(sequence, id)?;
                clip.remap = Some(remap.clone());
                clip.speed = Speed::NORMAL;
                ramped.push(id);
            }
        }
        if ramped.is_empty() {
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
        for id in in_step(sequence, self.clip) {
            let clip = ops::clip_mut(sequence, id)?;
            if clip.remap.is_none() {
                return Err(TimelineError::NothingToEdit);
            }
            let source = clip.source_out() - clip.source_in;
            clip.speed = Speed::stretch(source, clip.duration)
                .ok_or(TimelineError::SpeedOutOfRange(self.clip))?;
            clip.remap = None;
        }
        Ok(())
    }
}

/// Keeps the pitch of a remapped clip's sound, and of the sound playing in step with
/// it, as its speed changes, or lets the pitch follow the speed as on tape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetMaintainPitch {
    pub clip: ClipId,
    pub maintain: bool,
}

impl Edit for SetMaintainPitch {
    fn name(&self) -> &'static str {
        "Maintain Audio Pitch"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let ids = in_step(sequence, self.clip);
        let mut changed = false;
        for id in ids {
            let clip = ops::clip_mut(sequence, id)?;
            changed |= clip.maintain_pitch != self.maintain;
            clip.maintain_pitch = self.maintain;
        }
        if !changed {
            return Err(TimelineError::NothingToEdit);
        }
        Ok(())
    }
}

/// `id` and the clips linked to it that play the same source over the same span at the
/// same speed, so a speed curve given to it keeps their sound with its picture.
fn in_step(sequence: &Sequence, id: ClipId) -> Vec<ClipId> {
    let Some((_, clip)) = sequence.clip(id) else {
        return vec![id];
    };
    let same = |other: &Clip| {
        other.source == clip.source
            && other.start == clip.start
            && other.duration == clip.duration
            && other.source_in == clip.source_in
            && other.speed == clip.speed
            && other.remap == clip.remap
            && other.reverse == clip.reverse
            && !other.hold
    };
    let mut ids = vec![id];
    ids.extend(
        sequence
            .clips_with_links(&[id])
            .into_iter()
            .filter(|(_, other)| other.id != id && same(other))
            .map(|(_, other)| other.id),
    );
    ids
}

/// Changes a clip's speed curve, and those of the clips in step with it, with `change`,
/// then fits them to it so they still start and end on the same frames, without
/// running into the clip after any of them.
fn reshape(
    sequence: &mut Sequence,
    id: ClipId,
    change: impl Fn(&mut TimeRemap) -> bool,
) -> Result<(), TimelineError> {
    let ids = in_step(sequence, id);
    let room = ids
        .iter()
        .filter_map(|&id| {
            let (track, clip) = sequence.clip(id)?;
            sequence
                .track(track)?
                .clips
                .iter()
                .find(|next| next.start >= clip.end())
                .map(|next| next.start - clip.start)
        })
        .min();
    for id in ids {
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
    }
    Ok(())
}
