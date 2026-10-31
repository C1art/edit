use std::cmp::Reverse;

use crate::edit::ops;
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
        let mut changes: Vec<Change> = Vec::with_capacity(ids.len());
        // How much of each clip's new length the next clip on its track leaves no room for.
        let mut cut_short = Vec::with_capacity(ids.len());
        for &id in &ids {
            let (track, clip) = sequence.clip(id).ok_or(TimelineError::ClipNotFound(id))?;
            let source = clip.source_out() - clip.source_in;
            let rate = self.speed.times(clip.conform);
            let duration = rate.sequence_span(source).max(Ticks(1));
            let next = sequence.track(track).and_then(|track| {
                track
                    .clips
                    .iter()
                    .find(|other| other.start >= clip.end() && other.id != id)
            });
            let room = next.map_or(duration, |next| next.start - clip.start);
            cut_short.push((clip.link, (duration - room).max(Ticks::ZERO)));
            changes.push(Change {
                track,
                id,
                start: clip.start,
                end: clip.end(),
                by: duration - clip.duration,
            });
        }
        for (index, change) in changes.iter_mut().enumerate() {
            // Linked clips are cut short alike, so they still play the same source.
            let (link, own) = cut_short[index];
            let short = match link {
                Some(link) if !self.ripple => cut_short
                    .iter()
                    .filter(|(other, _)| *other == Some(link))
                    .map(|&(_, short)| short)
                    .max()
                    .unwrap_or(own),
                _ if !self.ripple => own,
                _ => Ticks::ZERO,
            };
            let (_, clip) = sequence
                .clip(change.id)
                .ok_or(TimelineError::ClipNotFound(change.id))?;
            let duration = (clip.duration + change.by - short).max(Ticks(1));
            change.by = duration - clip.duration;
            let clip = ops::clip_mut(sequence, change.id)?;
            clip.speed = self.speed;
            clip.remap = None;
            clip.duration = duration;
            clip.reverse = self.reverse;
            clip.maintain_pitch = self.maintain_pitch;
        }
        if self.ripple {
            ripple_changes(sequence, changes);
        }
        Ok(())
    }
}

/// A clip whose length changed by `by`, and where it was before.
struct Change {
    track: TrackRef,
    id: ClipId,
    start: Ticks,
    end: Ticks,
    by: Ticks,
}

/// Moves what follows the changed clips. Changes one after another on a track add up,
/// while clips side by side on different tracks, such as the two sides of an L-cut,
/// move what follows them by the largest of their changes, so it all stays in sync.
fn ripple_changes(sequence: &mut Sequence, mut changes: Vec<Change>) {
    let edited: Vec<TrackRef> = changes.iter().map(|change| change.track).collect();
    let rippled = ops::rippled_tracks(sequence, &edited);
    changes.sort_by_key(|change| change.start);
    let mut columns: Vec<Vec<Change>> = Vec::new();
    for change in changes {
        match columns.last_mut() {
            Some(column) if column.iter().any(|other| change.start < other.end) => {
                column.push(change);
            }
            _ => columns.push(vec![change]),
        }
    }
    // Latest first, so each column's ripple carries the later ones already moved.
    for mut column in columns.into_iter().rev() {
        column.sort_by_key(|change| Reverse(change.end));
        let ids: Vec<ClipId> = column.iter().map(|change| change.id).collect();
        // Each track's total change and the last end of its changed clips.
        let mut tracks: Vec<(TrackRef, Ticks, Ticks)> = Vec::new();
        for change in &column {
            if rippled.contains(&change.track) {
                ops::shift(
                    sequence,
                    &[change.track],
                    change.end,
                    change.by,
                    &[change.id],
                );
            }
            match tracks.iter_mut().find(|(track, ..)| *track == change.track) {
                Some((_, total, _)) => *total = *total + change.by,
                None => tracks.push((change.track, change.by, change.end)),
            }
        }
        let Some(shift) = tracks.iter().map(|&(_, total, _)| total).max() else {
            continue;
        };
        let end = column[0].end;
        for &track in &rippled {
            match tracks.iter().find(|(edited, ..)| *edited == track) {
                Some(&(_, total, last)) => {
                    ops::shift(sequence, &[track], last + total, shift - total, &ids);
                }
                None => ops::shift(sequence, &[track], end, shift, &ids),
            }
        }
        ops::ripple_markers_and_captions(sequence, end, shift);
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
