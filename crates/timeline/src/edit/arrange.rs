use std::ops::Range;

use crate::edit::ops::{self, Relinks};
use crate::{ClipId, Edit, Sequence, Ticks, TimelineError, TrackKind, TrackRef};

/// What a move does at either end, as Premiere's plain, Ctrl and Ctrl+Alt drags.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MoveMode {
    /// Leaves a gap behind and overwrites the destination.
    #[default]
    Overwrite,
    /// Leaves a gap behind and pushes the destination later to make room.
    Insert,
    /// Closes the gap behind, then inserts; neighbouring clips swap places.
    Rearrange,
}

/// Moves clips (and the clips linked to them) in time and across tracks of their kind.
/// Tracks are added as needed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveClips {
    pub clips: Vec<ClipId>,
    /// From where the clips are before the move, even when their gap closes.
    pub delta: Ticks,
    pub video_tracks: isize,
    pub audio_tracks: isize,
    pub mode: MoveMode,
}

impl Edit for MoveClips {
    fn name(&self) -> &'static str {
        match self.mode {
            MoveMode::Overwrite => "Move",
            MoveMode::Insert => "Insert Move",
            MoveMode::Rearrange => "Rearrange",
        }
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let ids = sequence.with_links(&self.clips);
        let moved = ops::remove(sequence, &ids);
        let Some(earliest) = moved.iter().map(|(_, clip)| clip.start).min() else {
            return Err(TimelineError::NothingToEdit);
        };
        let mut delta = self.delta;
        if self.mode == MoveMode::Rearrange {
            let gaps = ops::merge_ranges(
                moved
                    .iter()
                    .map(|(_, clip)| clip.start..clip.end())
                    .collect(),
            );
            let edited: Vec<TrackRef> = moved.iter().map(|&(track, _)| track).collect();
            close_gaps(sequence, &gaps, &edited);
            let destination = earliest + self.delta;
            for gap in gaps.iter().filter(|gap| gap.start < destination) {
                delta = delta - (gap.end.min(destination) - gap.start);
            }
        }
        let mut placed = Vec::with_capacity(moved.len());
        for (from, mut clip) in moved {
            let offset = match from.kind {
                TrackKind::Video => self.video_tracks,
                TrackKind::Audio => self.audio_tracks,
            };
            let index = from
                .index
                .checked_add_signed(offset)
                .ok_or(TimelineError::TrackNotFound(from))?;
            let to = TrackRef { index, ..from };
            let tracks = sequence.tracks_of_mut(to.kind);
            if tracks.len() <= index {
                tracks.resize_with(index + 1, Default::default);
            }
            clip.start = clip.start + delta;
            placed.push((to, clip));
        }
        let mut relinks = Relinks::default();
        match self.mode {
            MoveMode::Overwrite => {
                for (to, clip) in &placed {
                    ops::clear(sequence, *to, clip.start..clip.end(), &mut relinks)?;
                }
            }
            MoveMode::Insert | MoveMode::Rearrange => {
                let start = placed.iter().map(|(_, clip)| clip.start).min();
                let end = placed.iter().map(|(_, clip)| clip.end()).max();
                let (Some(start), Some(end)) = (start, end) else {
                    return Err(TimelineError::NothingToEdit);
                };
                let targets: Vec<TrackRef> = placed.iter().map(|&(to, _)| to).collect();
                let tracks = ops::rippled_tracks(sequence, &targets);
                ops::split(sequence, &tracks, start, &mut relinks);
                ops::shift(sequence, &tracks, start, end - start, &[]);
            }
        }
        for (to, clip) in placed {
            ops::insert(sequence, to, clip)?;
        }
        Ok(())
    }
}

/// Removes clips and the clips linked to them, leaving gaps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lift {
    pub clips: Vec<ClipId>,
}

impl Edit for Lift {
    fn name(&self) -> &'static str {
        "Lift"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let ids = sequence.with_links(&self.clips);
        match ops::remove(sequence, &ids).len() {
            0 => Err(TimelineError::NothingToEdit),
            _ => Ok(()),
        }
    }
}

/// Removes clips and the clips linked to them, then closes the gaps they leave on every
/// unlocked track. Fails if another clip sits in a gap and would be overlapped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RippleDelete {
    pub clips: Vec<ClipId>,
}

impl Edit for RippleDelete {
    fn name(&self) -> &'static str {
        "Ripple Delete"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let ids = sequence.with_links(&self.clips);
        let removed = ops::remove(sequence, &ids);
        if removed.is_empty() {
            return Err(TimelineError::NothingToEdit);
        }
        let gaps = ops::merge_ranges(
            removed
                .iter()
                .map(|(_, clip)| clip.start..clip.end())
                .collect(),
        );
        let edited: Vec<TrackRef> = removed.iter().map(|&(track, _)| track).collect();
        close_gaps(sequence, &gaps, &edited);
        Ok(())
    }
}

/// Closes the empty space around `at` on `track` by pulling everything after it earlier
/// on it and every sync-locked track.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloseGap {
    pub track: TrackRef,
    pub at: Ticks,
}

impl Edit for CloseGap {
    fn name(&self) -> &'static str {
        "Ripple Delete Gap"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let gap = sequence
            .track(self.track)
            .ok_or(TimelineError::TrackNotFound(self.track))?
            .gap_at(self.at)
            .ok_or(TimelineError::NothingToEdit)?;
        close_gaps(sequence, &[gap], &[self.track]);
        Ok(())
    }
}

/// `gaps` must be sorted and disjoint; they are closed from the last so earlier ones
/// keep their positions.
fn close_gaps(sequence: &mut Sequence, gaps: &[Range<Ticks>], edited: &[TrackRef]) {
    let tracks = ops::rippled_tracks(sequence, edited);
    for gap in gaps.iter().rev() {
        ops::shift(sequence, &tracks, gap.end, gap.start - gap.end, &[]);
    }
}
