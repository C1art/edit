use crate::edit::ops;
use crate::{ClipId, Edit, Sequence, Ticks, TimelineError, TrackRef};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Head,
    Tail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrimMode {
    /// Moves only the edge, leaving a gap or filling one.
    Normal,
    /// Keeps the clip's start and shifts everything after it on every unlocked track, so no
    /// gap opens and nothing is overwritten.
    Ripple,
}

/// Moves an edge of clips (and of the clips linked to them) by `delta`; positive is later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trim {
    pub clips: Vec<ClipId>,
    pub edge: Edge,
    pub delta: Ticks,
    pub mode: TrimMode,
}

impl Edit for Trim {
    fn name(&self) -> &'static str {
        match self.mode {
            TrimMode::Normal => "Trim",
            TrimMode::Ripple => "Ripple Trim",
        }
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let ids = sequence.with_links(&self.clips);
        if ids.is_empty() || self.delta == Ticks::ZERO {
            return Err(TimelineError::NothingToEdit);
        }
        let d = self.delta;
        // Each trimmed clip's track and the end it had, where a ripple starts on that track.
        let mut edit_points: Vec<(TrackRef, Ticks)> = Vec::with_capacity(ids.len());
        for &id in &ids {
            let (track, clip) = sequence.clip(id).ok_or(TimelineError::ClipNotFound(id))?;
            edit_points.push((track, clip.end()));
            let clip = ops::clip_mut(sequence, id)?;
            match (self.edge, self.mode) {
                (Edge::Head, TrimMode::Normal) => {
                    clip.start = clip.start + d;
                    clip.trim_head(d);
                }
                (Edge::Head, TrimMode::Ripple) => clip.trim_head(d),
                (Edge::Tail, _) => clip.trim_tail(d),
            }
        }
        if self.mode == TrimMode::Ripple {
            let shift = match self.edge {
                Edge::Head => Ticks::ZERO - d,
                Edge::Tail => d,
            };
            ripple(sequence, &edit_points, shift, &ids);
        }
        Ok(())
    }
}

/// Shifts each unlocked track from its edit point, or from the primary edit point on
/// tracks without a trimmed clip.
pub(super) fn ripple(
    sequence: &mut Sequence,
    edit_points: &[(TrackRef, Ticks)],
    shift: Ticks,
    skip: &[ClipId],
) {
    let Some(&(_, primary)) = edit_points.first() else {
        return;
    };
    for track in ops::unlocked_tracks(sequence) {
        let from = edit_points
            .iter()
            .find(|(edited, _)| *edited == track)
            .map_or(primary, |&(_, point)| point);
        ops::shift(sequence, &[track], from, shift, skip);
    }
}

/// Moves the cut at `at` on `tracks` by `delta`: the outgoing clip's tail and the incoming
/// clip's head move together, so nothing else in the sequence moves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Roll {
    pub at: Ticks,
    pub tracks: Vec<TrackRef>,
    pub delta: Ticks,
}

impl Edit for Roll {
    fn name(&self) -> &'static str {
        "Roll"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        if self.delta == Ticks::ZERO {
            return Err(TimelineError::NothingToEdit);
        }
        let mut rolled = false;
        for &track_ref in &self.tracks {
            for clip in &mut ops::track_mut(sequence, track_ref)?.clips {
                if clip.end() == self.at {
                    clip.trim_tail(self.delta);
                    rolled = true;
                } else if clip.start == self.at {
                    clip.start = clip.start + self.delta;
                    clip.trim_head(self.delta);
                    rolled = true;
                }
            }
        }
        if rolled {
            Ok(())
        } else {
            Err(TimelineError::NothingToEdit)
        }
    }
}

/// Changes which part of the source the clips show, keeping their place in the sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slip {
    pub clips: Vec<ClipId>,
    pub delta: Ticks,
}

impl Edit for Slip {
    fn name(&self) -> &'static str {
        "Slip"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let ids = sequence.with_links(&self.clips);
        if ids.is_empty() || self.delta == Ticks::ZERO {
            return Err(TimelineError::NothingToEdit);
        }
        for id in ids {
            let clip = ops::clip_mut(sequence, id)?;
            clip.source_in = clip.source_in + self.delta;
        }
        Ok(())
    }
}

/// Moves clips along their tracks while the adjacent clips give and take the difference:
/// the one before is extended or shortened at its tail and the one after at its head.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slide {
    pub clips: Vec<ClipId>,
    pub delta: Ticks,
}

impl Edit for Slide {
    fn name(&self) -> &'static str {
        "Slide"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let ids = sequence.with_links(&self.clips);
        if ids.is_empty() || self.delta == Ticks::ZERO {
            return Err(TimelineError::NothingToEdit);
        }
        let d = self.delta;
        for &id in &ids {
            let (track_ref, clip) = sequence.clip(id).ok_or(TimelineError::ClipNotFound(id))?;
            let (start, end) = (clip.start, clip.end());
            for neighbor in &mut ops::track_mut(sequence, track_ref)?.clips {
                if neighbor.id == id {
                    neighbor.start = neighbor.start + d;
                } else if ids.contains(&neighbor.id) {
                    continue;
                } else if neighbor.end() == start {
                    neighbor.trim_tail(d);
                } else if neighbor.start == end {
                    neighbor.start = neighbor.start + d;
                    neighbor.trim_head(d);
                }
            }
        }
        Ok(())
    }
}
