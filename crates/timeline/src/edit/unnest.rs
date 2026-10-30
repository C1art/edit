//! Unnest: a nested sequence's clips put back in the place of a clip of it, so they can
//! be edited where they play.

use std::collections::HashMap;
use std::ops::Range;

use super::add_tracks::renumber_layers;
use super::ops;
use crate::{
    Clip, ClipId, Edit, Gain, LinkId, Nest, PinnedEnds, Sequence, Ticks, TimelineError, Track,
    TrackKind, TrackRef,
};

/// Replaces each nest clip among `clips` and those linked to them with the nest's clips
/// over its span. Only nests that play as they would on their own are unnested: at
/// normal speed, without effects or transitions on the clip, and with a plain mix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unnest {
    pub clips: Vec<ClipId>,
}

/// The new link of each piece, by the link of the clip it came out of and its own.
type Relinks = HashMap<(Option<LinkId>, LinkId), LinkId>;

impl Edit for Unnest {
    fn name(&self) -> &'static str {
        "Unnest"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let ids = sequence.with_links(&self.clips);
        let blocked: Vec<_> = ids
            .iter()
            .filter(|&&id| !unnests(sequence, id))
            .filter_map(|&id| sequence.clip(id).map(|(_, clip)| (id, clip.link)))
            .collect();
        // A clip whose linked partner can't be unnested stays too, keeping the pair whole.
        let unnested: Vec<ClipId> = ids
            .into_iter()
            .filter(|&id| {
                let link = sequence.clip(id).and_then(|(_, clip)| clip.link);
                !blocked.iter().any(|&(other, other_link)| {
                    other == id || (link.is_some() && other_link == link)
                })
            })
            .collect();
        if unnested.is_empty() {
            return Err(TimelineError::NothingToEdit);
        }
        let mut relinks = Relinks::new();
        for id in unnested {
            unnest(sequence, id, &mut relinks)?;
        }
        Ok(())
    }
}

fn unnests(sequence: &Sequence, id: ClipId) -> bool {
    let Some((track, clip)) = sequence.clip(id) else {
        return false;
    };
    let Some(nest) = sequence
        .media_item(clip.source)
        .and_then(|item| item.nest.as_ref())
    else {
        return false;
    };
    let reads_tracks = || {
        nest.video
            .iter()
            .flat_map(|track| &track.clips)
            .flat_map(|clip| &clip.look.effects)
            .any(|effect| effect.other_track().is_some())
    };
    !nest.multicam
        && plays_plainly(clip)
        && match track.kind {
            TrackKind::Video => !reads_tracks(),
            TrackKind::Audio => mixes_plainly(nest),
        }
}

/// Whether the clip shows its source unchanged, as a nest seen on its own would.
fn plays_plainly(clip: &Clip) -> bool {
    let plain = Clip {
        link: clip.link,
        group: clip.group,
        label: clip.label,
        name: clip.name.clone(),
        markers: clip.markers.clone(),
        audio_type: clip.audio_type,
        interpolation: clip.interpolation,
        ..Clip::new(
            clip.id,
            clip.source,
            clip.start,
            clip.duration,
            clip.source_in,
        )
    };
    *clip == plain
}

/// Whether the nest's tracks and master leave its sound as the clips make it.
fn mixes_plainly(nest: &Nest) -> bool {
    let settings = |track: &Track| Track {
        clips: Vec::new(),
        name: None,
        muted: false,
        locked: false,
        sync_unlocked: false,
        solo: false,
        ..track.clone()
    };
    nest.master == Gain::UNITY
        && nest.master_effects.is_empty()
        && nest
            .audio
            .iter()
            .all(|track| settings(track) == Track::default())
}

fn unnest(sequence: &mut Sequence, id: ClipId, relinks: &mut Relinks) -> Result<(), TimelineError> {
    let Some((track, clip)) = sequence.clip(id) else {
        return Ok(());
    };
    let clip = clip.clone();
    let Some(nest) = sequence
        .media_item(clip.source)
        .and_then(|item| item.nest.as_ref())
    else {
        return Ok(());
    };
    let layers: Vec<Vec<Clip>> = heard(nest, track.kind)
        .map(|inner| pieces(inner, &clip))
        .filter(|layer| !layer.is_empty())
        .collect();
    ops::remove(sequence, &[id]);
    let span = clip.start..clip.end();
    let mut index = track.index;
    for (layer, pieces) in layers.into_iter().enumerate() {
        if layer > 0 {
            index += 1;
            make_room(sequence, TrackRef { index, ..track }, track.index, &span);
        }
        for mut piece in pieces {
            piece.id = ClipId(sequence.ids.next());
            piece.group = None;
            piece.link = piece.link.map(|inner| {
                *relinks
                    .entry((clip.link, inner))
                    .or_insert_with(|| LinkId(sequence.ids.next()))
            });
            ops::insert(sequence, TrackRef { index, ..track }, piece)?;
        }
    }
    Ok(())
}

/// The nest's tracks of `kind` that play: the soloed ones if any are, else those on.
fn heard(nest: &Nest, kind: TrackKind) -> impl Iterator<Item = &Track> {
    let tracks = match kind {
        TrackKind::Video => &nest.video,
        TrackKind::Audio => &nest.audio,
    };
    let soloing = tracks.iter().any(|track| track.solo);
    tracks
        .iter()
        .filter(move |track| !track.muted && (!soloing || track.solo))
}

/// What of `inner` plays in `clip`'s span, at the sequence times it plays there.
fn pieces(inner: &Track, clip: &Clip) -> Vec<Clip> {
    let (from, to) = (clip.source_in, clip.source_in + clip.duration);
    inner
        .clips
        .iter()
        .filter(|piece| piece.start < to && from < piece.end())
        .map(|piece| {
            let mut piece = Clip::clone(piece);
            // Cut from the nest rather than trimmed, so no keyframes move.
            piece.pinned = PinnedEnds::default();
            if piece.start < from {
                piece.trim_head(from - piece.start);
                piece.start = from;
                piece.transition_in = None;
            }
            if piece.end() > to {
                piece.trim_tail(to - piece.end());
                piece.transition_out = None;
            }
            piece.start = clip.start + (piece.start - from);
            piece
        })
        .collect()
}

/// Keeps `track` if it is free over `span` and plays like the track the nest clip was
/// on; otherwise puts a track like that one there, moving the rest up.
fn make_room(sequence: &mut Sequence, track: TrackRef, base: usize, span: &Range<Ticks>) {
    let tracks = sequence.tracks_of(track.kind);
    let settings = |track: &Track| Track {
        clips: Vec::new(),
        name: None,
        sync_unlocked: false,
        ..track.clone()
    };
    let Some(base) = tracks.get(base).map(settings) else {
        return;
    };
    let free = tracks.get(track.index).is_some_and(|existing| {
        settings(existing) == base
            && !existing.locked
            && !existing
                .clips
                .iter()
                .any(|clip| clip.start < span.end && span.start < clip.end())
    });
    if free {
        return;
    }
    let at = track.index.min(tracks.len());
    sequence.tracks_of_mut(track.kind).insert(at, base);
    if track.kind == TrackKind::Video {
        renumber_layers(sequence, |layer| {
            Some(if layer >= at { layer + 1 } else { layer })
        });
    }
}
