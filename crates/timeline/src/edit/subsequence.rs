use std::ops::Range;
use std::sync::Arc;

use super::arrange::close_gaps;
use super::media::renumber;
use super::ops::{self, Relinks};
use crate::{
    Clip, ClipId, Edit, MediaItem, Nest, Sequence, SourceId, Ticks, TimelineError, TrackRef,
};

/// Copies the clips between the in and out marks, or `clips` and those linked to them,
/// into a new sequence in the pool, cut to that span and starting at zero, as
/// Premiere's Make Subsequence does. The sequence itself is left as it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MakeSubsequence {
    pub clips: Vec<ClipId>,
    pub name: String,
}

impl Edit for MakeSubsequence {
    fn name(&self) -> &'static str {
        "Make Subsequence"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let mut copy = sequence.clone();
        let range = if self.clips.is_empty() {
            let range = sequence.marks.range(sequence.duration());
            if !sequence.marks.is_set() || range.is_empty() {
                return Err(TimelineError::NothingToEdit);
            }
            range
        } else {
            let ids = sequence.with_links(&self.clips);
            for (_, track) in copy.tracks_mut() {
                track.clips.retain(|clip| ids.contains(&clip.id));
            }
            span(&copy).ok_or(TimelineError::NothingToEdit)?
        };
        let tracks: Vec<TrackRef> = copy.tracks().map(|(track, _)| track).collect();
        let mut relinks = Relinks::default();
        for &track in &tracks {
            ops::clear(&mut copy, track, Ticks::ZERO..range.start, &mut relinks)?;
            ops::clear(&mut copy, track, range.end..Ticks(i64::MAX), &mut relinks)?;
        }
        if span(&copy).is_none() {
            return Err(TimelineError::NothingToEdit);
        }
        ops::shift(
            &mut copy,
            &tracks,
            Ticks::ZERO,
            Ticks::ZERO - range.start,
            &[],
        );
        let markers = copy
            .markers
            .into_iter()
            .filter(|marker| range.contains(&marker.at))
            .map(|marker| crate::Marker {
                at: marker.at - range.start,
                ..marker
            })
            .collect();
        let mut nest = Nest {
            video: copy.video,
            audio: copy.audio,
            markers,
            captions: crate::Captions {
                items: copy.captions.within(range.clone()),
                ..copy.captions
            },
            ..Nest::default()
        };
        renumber(&mut nest, &mut sequence.ids);
        let id = SourceId(sequence.ids.next());
        Arc::make_mut(&mut sequence.media).push(MediaItem {
            id,
            name: self.name.clone(),
            path: Default::default(),
            duration: range.end - range.start,
            video: Some(sequence.format),
            has_audio: true,
            proxy: None,
            bin: None,
            generator: None,
            nest: Some(nest),
            subclip: None,
            interpreted_rate: None,
            interpreted_aspect: None,
            interpreted_color: None,
            interpreted_fields: None,
            alpha: Default::default(),
            log: Default::default(),
            offline: false,
            still: false,
            channel_map: None,
            placed: None,
            frame_fit: None,
            timecode: None,
            poster: None,
            transcript: None,
        });
        sequence.fit_nest(id);
        Ok(())
    }
}

/// Closes every stretch of the sequence, from its start, where no track has a clip,
/// pulling what follows earlier on every unlocked track so they stay in sync.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloseGaps;

impl Edit for CloseGaps {
    fn name(&self) -> &'static str {
        "Close Gap"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let gaps = unused_time(
            sequence
                .tracks()
                .flat_map(|(_, track)| track.clips.iter().map(Arc::as_ref)),
        );
        if gaps.is_empty() {
            return Err(TimelineError::NothingToEdit);
        }
        let tracks: Vec<TrackRef> = sequence.tracks().map(|(track, _)| track).collect();
        close_gaps(sequence, &gaps, &tracks);
        Ok(())
    }
}

/// The stretches from the sequence's start to the last of `clips` that none of them uses.
pub fn unused_time<'a>(clips: impl IntoIterator<Item = &'a Clip>) -> Vec<Range<Ticks>> {
    let filled = ops::merge_ranges(
        clips
            .into_iter()
            .map(|clip| clip.start..clip.end())
            .collect(),
    );
    filled
        .iter()
        .scan(Ticks::ZERO, |end, range| {
            let gap = *end..range.start;
            *end = range.end;
            Some(gap)
        })
        .filter(|gap| !gap.is_empty())
        .collect()
}

/// From the earliest clip's start to the latest's end.
fn span(sequence: &Sequence) -> Option<Range<Ticks>> {
    let clips = || {
        sequence
            .tracks()
            .flat_map(|(_, track)| track.clips.iter().map(Arc::as_ref))
    };
    let start = clips().map(|clip| clip.start).min()?;
    let end = clips().map(|clip| clip.end()).max()?;
    Some(start..end)
}
