use std::collections::HashSet;

use crate::edit::ops;
use crate::{Clip, ClipId, Edit, Sequence, Ticks, TimelineError, TrackRef};

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
        let mut edit_points: Vec<(ClipId, TrackRef, Ticks)> = Vec::with_capacity(ids.len());
        for (track, clip) in ops::clips_mut(sequence, &ids) {
            edit_points.push((clip.id, track, clip.end()));
            match (self.edge, self.mode) {
                (Edge::Head, TrimMode::Normal) => {
                    clip.start = clip.start + d;
                    clip.trim_head(d);
                }
                (Edge::Head, TrimMode::Ripple) => clip.trim_head(d),
                (Edge::Tail, _) => clip.trim_tail(d),
            }
        }
        // The first in id order is the primary edit point, wherever its track is.
        edit_points.sort_unstable_by_key(|&(id, ..)| id);
        let edit_points: Vec<(TrackRef, Ticks)> = edit_points
            .into_iter()
            .map(|(_, track, end)| (track, end))
            .collect();
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

/// Shifts each edited and sync-locked track from its edit point, or from the primary edit point on
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
    let edited: Vec<TrackRef> = edit_points.iter().map(|&(track, _)| track).collect();
    for track in ops::rippled_tracks(sequence, &edited) {
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
        for (_, clip) in ops::clips_mut(sequence, &ids) {
            clip.source_in = clip.source_in + self.delta;
            if let Some(remap) = &mut clip.remap {
                remap.shift(Ticks::ZERO, self.delta);
            }
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
        let ids: HashSet<ClipId> = sequence.with_links(&self.clips).into_iter().collect();
        if ids.is_empty() || self.delta == Ticks::ZERO {
            return Err(TimelineError::NothingToEdit);
        }
        let d = self.delta;
        for (_, track) in sequence.tracks_mut() {
            let slid = track.clips.iter().filter(|clip| ids.contains(&clip.id));
            let starts: HashSet<Ticks> = slid.clone().map(|clip| clip.start).collect();
            let ends: HashSet<Ticks> = slid.map(Clip::end).collect();
            for clip in &mut track.clips {
                if ids.contains(&clip.id) {
                    clip.start = clip.start + d;
                    continue;
                }
                // A clip between two slid ones gives at both ends.
                let (start, end) = (clip.start, clip.end());
                if starts.contains(&end) {
                    clip.trim_tail(d);
                }
                if ends.contains(&start) {
                    clip.start = clip.start + d;
                    clip.trim_head(d);
                }
            }
        }
        Ok(())
    }
}
