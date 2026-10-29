use crate::edit::ops;
use crate::edit::trim::ripple;
use crate::{
    ClipId, Edge, Edit, Sequence, Speed, Ticks, TimeInterpolation, TimelineError, TrackRef,
};

/// Plays clips at `speed` from the same in point, so their duration changes; reversing
/// plays the same source range backwards. Without `ripple`, a clip that grows stops at
/// the next clip on its track.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetClipSpeed {
    pub clips: Vec<ClipId>,
    pub speed: Speed,
    /// Moves everything after the clips by the change in their length.
    pub ripple: bool,
    pub reverse: bool,
    pub maintain_pitch: bool,
}

impl Edit for SetClipSpeed {
    fn name(&self) -> &'static str {
        "Speed/Duration"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        // A frame hold has no speed; its length is set by trimming.
        let ids: Vec<ClipId> = sequence
            .with_links(&self.clips)
            .into_iter()
            .filter(|&id| sequence.clip(id).is_some_and(|(_, clip)| !clip.hold))
            .collect();
        let Some(&first) = ids.first() else {
            return Err(TimelineError::NothingToEdit);
        };
        if !self.speed.is_in_range() {
            return Err(TimelineError::SpeedOutOfRange(first));
        }
        let mut edit_points: Vec<(TrackRef, Ticks)> = Vec::with_capacity(ids.len());
        let mut shift = None;
        for &id in &ids {
            let (track_ref, clip) = sequence.clip(id).ok_or(TimelineError::ClipNotFound(id))?;
            let source = clip.source_out() - clip.source_in;
            let rate = self.speed.times(clip.conform);
            let mut duration = rate.sequence_span(source).max(Ticks(1));
            if !self.ripple {
                let next = sequence.track(track_ref).and_then(|track| {
                    track
                        .clips
                        .iter()
                        .find(|other| other.start >= clip.end() && other.id != id)
                });
                if let Some(next) = next {
                    duration = duration.min(next.start - clip.start);
                }
            }
            edit_points.push((track_ref, clip.end()));
            shift.get_or_insert(duration - clip.duration);
            let clip = ops::clip_mut(sequence, id)?;
            clip.speed = self.speed;
            clip.remap = None;
            clip.duration = duration;
            clip.reverse = self.reverse;
            clip.maintain_pitch = self.maintain_pitch;
        }
        if let (true, Some(shift)) = (self.ripple, shift) {
            ripple(sequence, &edit_points, shift, &ids);
        }
        Ok(())
    }
}

/// Drags an edge of clips while they keep playing the same source, so the speed changes
/// to fit: the rate stretch tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateStretch {
    pub clips: Vec<ClipId>,
    pub edge: Edge,
    pub delta: Ticks,
}

impl Edit for RateStretch {
    fn name(&self) -> &'static str {
        "Rate Stretch"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let ids = sequence.with_links(&self.clips);
        if ids.is_empty() || self.delta == Ticks::ZERO {
            return Err(TimelineError::NothingToEdit);
        }
        for (_, clip) in ops::clips_mut(sequence, &ids) {
            let source = clip.source_out() - clip.source_in;
            let duration = match self.edge {
                Edge::Head => clip.duration - self.delta,
                Edge::Tail => clip.duration + self.delta,
            };
            if let Some(remap) = &mut clip.remap {
                remap.stretch(clip.duration, duration);
            } else if !clip.hold {
                let rate =
                    Speed::stretch(source, duration).ok_or(TimelineError::NonPositiveDuration)?;
                clip.speed = rate.over(clip.conform);
            }
            clip.duration = duration;
            if self.edge == Edge::Head {
                clip.start = clip.start + self.delta;
            }
        }
        Ok(())
    }
}

/// Sets how clips show the source between their frames.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetTimeInterpolation {
    pub clips: Vec<ClipId>,
    pub interpolation: TimeInterpolation,
}

impl Edit for SetTimeInterpolation {
    fn name(&self) -> &'static str {
        "Time Interpolation"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let mut changed = false;
        for &id in &self.clips {
            let clip = ops::clip_mut(sequence, id)?;
            changed |= clip.interpolation != self.interpolation;
            clip.interpolation = self.interpolation;
        }
        if changed {
            Ok(())
        } else {
            Err(TimelineError::NothingToEdit)
        }
    }
}
