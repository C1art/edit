//! Building blocks shared by the edits. They don't check invariants themselves; the
//! edited sequence is validated as a whole afterwards.

use std::collections::HashMap;
use std::ops::Range;

use crate::{Clip, ClipId, LinkId, Sequence, Ticks, TimelineError, TrackRef};

/// Splitting linked clips gives every right-hand piece of the same link a shared new
/// link, so the pieces on either side of a cut stay linked to each other.
#[derive(Debug, Default)]
pub(crate) struct Relinks(HashMap<LinkId, LinkId>);

pub(crate) fn unlocked_tracks(sequence: &Sequence) -> Vec<TrackRef> {
    sequence
        .tracks()
        .filter(|(_, track)| !track.locked)
        .map(|(track_ref, _)| track_ref)
        .collect()
}

pub(crate) fn track_mut(
    sequence: &mut Sequence,
    track: TrackRef,
) -> Result<&mut crate::Track, TimelineError> {
    sequence
        .track_mut(track)
        .ok_or(TimelineError::TrackNotFound(track))
}

pub(crate) fn clip_mut(sequence: &mut Sequence, id: ClipId) -> Result<&mut Clip, TimelineError> {
    sequence
        .tracks_mut()
        .find_map(|(_, track)| track.clips.iter_mut().find(|clip| clip.id == id))
        .ok_or(TimelineError::ClipNotFound(id))
}

/// Cuts the clip strictly spanning `at` on each of `tracks`; returns how many were cut.
pub(crate) fn split(
    sequence: &mut Sequence,
    tracks: &[TrackRef],
    at: Ticks,
    relinks: &mut Relinks,
) -> usize {
    let mut cuts = 0;
    for &track_ref in tracks {
        let Some(track) = sequence.track(track_ref) else {
            continue;
        };
        let Some(index) = track
            .clips
            .iter()
            .position(|clip| clip.start < at && at < clip.end())
        else {
            continue;
        };
        let left = track.clips[index].clone();
        let link = left.link.map(|link| {
            *relinks
                .0
                .entry(link)
                .or_insert_with(|| LinkId(sequence.ids.next()))
        });
        let right = Clip {
            id: ClipId(sequence.ids.next()),
            start: at,
            duration: left.end() - at,
            source_in: left.source_time(at),
            link,
            // The cut becomes a plain cut; the transitions stay at the outer ends.
            transition_in: None,
            ..left
        };
        let Some(track) = sequence.track_mut(track_ref) else {
            continue;
        };
        track.clips[index].duration = at - track.clips[index].start;
        track.clips[index].transition_out = None;
        track.clips.insert(index + 1, right);
        cuts += 1;
    }
    cuts
}

/// Removes everything within `range` on `track`, cutting clips that straddle its ends.
pub(crate) fn clear(
    sequence: &mut Sequence,
    track: TrackRef,
    range: Range<Ticks>,
    relinks: &mut Relinks,
) -> Result<(), TimelineError> {
    split(sequence, &[track], range.start, relinks);
    split(sequence, &[track], range.end, relinks);
    track_mut(sequence, track)?
        .clips
        .retain(|clip| clip.end() <= range.start || clip.start >= range.end);
    Ok(())
}

/// Moves every clip on `tracks` that starts at or after `from` by `delta`, except `skip`.
pub(crate) fn shift(
    sequence: &mut Sequence,
    tracks: &[TrackRef],
    from: Ticks,
    delta: Ticks,
    skip: &[ClipId],
) {
    for &track_ref in tracks {
        let Some(track) = sequence.track_mut(track_ref) else {
            continue;
        };
        track
            .clips
            .iter_mut()
            .filter(|clip| clip.start >= from && !skip.contains(&clip.id))
            .for_each(|clip| clip.start = clip.start + delta);
    }
}

/// Inserts `clip` in start order without checking for overlaps.
pub(crate) fn insert(
    sequence: &mut Sequence,
    track: TrackRef,
    clip: Clip,
) -> Result<(), TimelineError> {
    let clips = &mut track_mut(sequence, track)?.clips;
    let index = clips.partition_point(|existing| existing.start <= clip.start);
    clips.insert(index, clip);
    Ok(())
}

/// Takes the clips out of the sequence, returning each with the track it was on.
pub(crate) fn remove(sequence: &mut Sequence, ids: &[ClipId]) -> Vec<(TrackRef, Clip)> {
    let mut removed = Vec::new();
    for (track_ref, track) in sequence.tracks_mut() {
        let (taken, kept) = std::mem::take(&mut track.clips)
            .into_iter()
            .partition(|clip| ids.contains(&clip.id));
        track.clips = kept;
        removed.extend(taken.into_iter().map(|clip: Clip| (track_ref, clip)));
    }
    removed
}

/// Sorts and merges ranges that overlap or touch.
pub(crate) fn merge_ranges(mut ranges: Vec<Range<Ticks>>) -> Vec<Range<Ticks>> {
    ranges.sort_unstable_by_key(|range| range.start);
    let mut merged: Vec<Range<Ticks>> = Vec::with_capacity(ranges.len());
    for range in ranges {
        match merged.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => merged.push(range),
        }
    }
    merged
}
