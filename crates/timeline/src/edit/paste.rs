use std::collections::HashMap;

use crate::edit::ops::{self, Relinks};
use crate::{
    Clip, ClipId, Edit, GroupId, LinkId, Sequence, Ticks, TimelineError, Track, TrackKind, TrackRef,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteMode {
    /// Pushes everything at and after the paste point later on every unlocked track.
    Insert,
    /// Replaces whatever is in the way on the target tracks.
    Overwrite,
    /// Moves each kind's clips up together to the first tracks free for their whole spans,
    /// adding tracks when none are, so nothing already placed is touched.
    OntoFreeTracks,
}

/// Copied clips with their tracks, timed from the start of the earliest one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clipboard {
    clips: Vec<(TrackRef, Clip)>,
}

impl Clipboard {
    /// The clips `ids` and those linked to them, or `None` when none exist.
    pub fn copy(sequence: &Sequence, ids: &[ClipId]) -> Option<Self> {
        let mut clips: Vec<(TrackRef, Clip)> = sequence
            .with_links(ids)
            .into_iter()
            .filter_map(|id| sequence.clip(id))
            .map(|(track, clip)| (track, clip.clone()))
            .collect();
        let origin = clips.iter().map(|(_, clip)| clip.start).min()?;
        for (_, clip) in &mut clips {
            clip.start = clip.start - origin;
        }
        Some(Self { clips })
    }

    pub fn clip_count(&self) -> usize {
        self.clips.len()
    }

    /// The earliest clip of `kind` on the lowest-numbered track it was copied from.
    pub fn first_clip(&self, kind: TrackKind) -> Option<&Clip> {
        self.clips
            .iter()
            .filter(|(track, _)| track.kind == kind)
            .min_by_key(|(track, clip)| (track.index, clip.start))
            .map(|(_, clip)| clip)
    }

    /// The lowest-numbered track of `kind` the clips were copied from.
    pub fn first_track(&self, kind: TrackKind) -> Option<usize> {
        self.clips
            .iter()
            .filter(|(track, _)| track.kind == kind)
            .map(|(track, _)| track.index)
            .min()
    }

    /// From the earliest start to the latest end.
    pub fn duration(&self) -> Ticks {
        self.clips
            .iter()
            .map(|(_, clip)| clip.end())
            .max()
            .unwrap_or(Ticks::ZERO)
    }
}

/// Places copied clips at `at`, on the tracks they were copied from shifted by the track
/// offsets, with new ids and their own links.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasteClips {
    pub clipboard: Clipboard,
    pub at: Ticks,
    pub mode: PasteMode,
    /// Added to each video clip's track index, e.g. to paste onto the targeted track.
    pub video_offset: isize,
    pub audio_offset: isize,
}

impl PasteClips {
    fn target(&self, track: TrackRef, raise: [usize; 2]) -> Result<TrackRef, TimelineError> {
        let (offset, raise) = match track.kind {
            TrackKind::Video => (self.video_offset, raise[0]),
            TrackKind::Audio => (self.audio_offset, raise[1]),
        };
        track
            .index
            .checked_add_signed(offset)
            .map(|index| TrackRef {
                index: index + raise,
                ..track
            })
            .ok_or(TimelineError::TrackNotFound(track))
    }

    /// How many tracks above their offset tracks the clips of `kind` must go to land only
    /// on free, unlocked space.
    fn free_raise(&self, sequence: &Sequence, kind: TrackKind) -> Result<usize, TimelineError> {
        let clips: Vec<(TrackRef, &Clip)> = self
            .clipboard
            .clips
            .iter()
            .filter(|(track, _)| track.kind == kind)
            .map(|(track, clip)| Ok((self.target(*track, [0, 0])?, clip)))
            .collect::<Result<_, TimelineError>>()?;
        let tracks = sequence.tracks_of(kind);
        let fits = |raise: usize| {
            clips.iter().all(|(track, clip)| {
                let start = self.at + clip.start;
                let end = start + clip.duration;
                tracks.get(track.index + raise).is_none_or(|track| {
                    !track.locked
                        && track
                            .clips
                            .iter()
                            .all(|placed| placed.end() <= start || end <= placed.start)
                })
            })
        };
        Ok((0..=tracks.len()).find(|&raise| fits(raise)).unwrap_or(0))
    }
}

impl Edit for PasteClips {
    fn name(&self) -> &'static str {
        match self.mode {
            PasteMode::Insert => "Paste Insert",
            PasteMode::Overwrite | PasteMode::OntoFreeTracks => "Paste",
        }
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let mut relinks = Relinks::default();
        let raise = match self.mode {
            PasteMode::OntoFreeTracks => [
                self.free_raise(sequence, TrackKind::Video)?,
                self.free_raise(sequence, TrackKind::Audio)?,
            ],
            PasteMode::Insert | PasteMode::Overwrite => [0, 0],
        };
        let placed: Vec<(TrackRef, &Clip)> = self
            .clipboard
            .clips
            .iter()
            .map(|(track, clip)| Ok((self.target(*track, raise)?, clip)))
            .collect::<Result<_, TimelineError>>()?;
        if placed.is_empty() {
            return Err(TimelineError::NothingToEdit);
        }
        match self.mode {
            PasteMode::Insert => {
                let edited: Vec<TrackRef> = placed.iter().map(|&(track, _)| track).collect();
                let tracks = ops::rippled_tracks(sequence, &edited);
                ops::split(sequence, &tracks, self.at, &mut relinks);
                let duration = self.clipboard.duration();
                ops::shift(sequence, &tracks, self.at, duration, &[]);
                ops::ripple_markers(sequence, self.at, duration);
            }
            PasteMode::OntoFreeTracks => {
                for &(track, _) in &placed {
                    let tracks = sequence.tracks_of_mut(track.kind);
                    if tracks.len() <= track.index {
                        tracks.resize_with(track.index + 1, Track::default);
                    }
                }
            }
            PasteMode::Overwrite => {
                for &(track, clip) in &placed {
                    let start = self.at + clip.start;
                    ops::clear(sequence, track, start..start + clip.duration, &mut relinks)?;
                }
            }
        }
        let mut links: HashMap<LinkId, LinkId> = HashMap::new();
        let mut groups: HashMap<GroupId, GroupId> = HashMap::new();
        for (track, clip) in placed {
            let link = clip.link.map(|link| {
                *links
                    .entry(link)
                    .or_insert_with(|| LinkId(sequence.ids.next()))
            });
            let group = clip.group.map(|group| {
                *groups
                    .entry(group)
                    .or_insert_with(|| GroupId(sequence.ids.next()))
            });
            let pasted = Clip {
                id: ClipId(sequence.ids.next()),
                start: self.at + clip.start,
                link,
                group,
                ..clip.clone()
            };
            ops::insert(sequence, track, pasted)?;
        }
        Ok(())
    }
}
