use std::collections::HashMap;

use crate::edit::ops::{self, Relinks};
use crate::{
    Clip, ClipId, Edit, GroupId, LinkId, PlaceMode, Sequence, Ticks, TimelineError, TrackKind,
    TrackRef,
};

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
    pub mode: PlaceMode,
    /// Added to each video clip's track index, e.g. to paste onto the targeted track.
    pub video_offset: isize,
    pub audio_offset: isize,
}

impl PasteClips {
    fn target(&self, track: TrackRef) -> Result<TrackRef, TimelineError> {
        let offset = match track.kind {
            TrackKind::Video => self.video_offset,
            TrackKind::Audio => self.audio_offset,
        };
        track
            .index
            .checked_add_signed(offset)
            .map(|index| TrackRef { index, ..track })
            .ok_or(TimelineError::TrackNotFound(track))
    }
}

impl Edit for PasteClips {
    fn name(&self) -> &'static str {
        match self.mode {
            PlaceMode::Insert => "Paste Insert",
            PlaceMode::Overwrite => "Paste",
        }
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let mut relinks = Relinks::default();
        let placed: Vec<(TrackRef, &Clip)> = self
            .clipboard
            .clips
            .iter()
            .map(|(track, clip)| Ok((self.target(*track)?, clip)))
            .collect::<Result<_, TimelineError>>()?;
        if placed.is_empty() {
            return Err(TimelineError::NothingToEdit);
        }
        match self.mode {
            PlaceMode::Insert => {
                let edited: Vec<TrackRef> = placed.iter().map(|&(track, _)| track).collect();
                let tracks = ops::rippled_tracks(sequence, &edited);
                ops::split(sequence, &tracks, self.at, &mut relinks);
                let duration = self.clipboard.duration();
                ops::shift(sequence, &tracks, self.at, duration, &[]);
                ops::ripple_markers(sequence, self.at, duration);
            }
            PlaceMode::Overwrite => {
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
