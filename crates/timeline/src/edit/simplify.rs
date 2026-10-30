use std::ops::Range;

use super::ops;
use crate::{
    Clip, ClipId, DeleteEmptyTracks, Edit, Sequence, Ticks, TimelineError, Track, TrackKind,
    TrackRef,
};

/// Tidies a sequence without changing what it plays, as Premiere's Simplify Sequence
/// does before handing a cut on: disabled clips and clips on tracks turned off go, clips
/// drop to the lowest track they can, and tracks left empty go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimplifySequence {
    pub remove_disabled: bool,
    /// Clips on video tracks hidden or audio tracks silenced.
    pub remove_on_tracks_off: bool,
    pub lower_clips: bool,
    pub delete_empty_tracks: bool,
}

impl Edit for SimplifySequence {
    fn name(&self) -> &'static str {
        "Simplify Sequence"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let unplayed: Vec<ClipId> = sequence
            .tracks()
            .filter(|(_, track)| !track.locked)
            .flat_map(|(_, track)| {
                let off = self.remove_on_tracks_off && track.muted;
                track
                    .clips
                    .iter()
                    .filter(move |clip| off || (self.remove_disabled && clip.disabled))
                    .map(|clip| clip.id)
            })
            .collect();
        let mut changed = !ops::remove(sequence, &unplayed).is_empty();
        if self.lower_clips {
            for kind in [TrackKind::Video, TrackKind::Audio] {
                changed |= lower_clips(sequence, kind)?;
            }
        }
        if self.delete_empty_tracks {
            let kinds = vec![TrackKind::Video, TrackKind::Audio];
            match (DeleteEmptyTracks { kinds }).edit(sequence) {
                Ok(()) => changed = true,
                Err(TimelineError::NothingToEdit) => {}
                Err(error) => return Err(error),
            }
        }
        if !changed {
            return Err(TimelineError::NothingToEdit);
        }
        Ok(())
    }
}

/// Moves each clip down while the track below is free over it and plays the same way,
/// so nothing passes beneath another clip and the picture and mix stay as they were.
/// Video stays put when an effect reads other tracks by their place in the stack.
fn lower_clips(sequence: &mut Sequence, kind: TrackKind) -> Result<bool, TimelineError> {
    let reads_tracks = sequence
        .tracks_of(kind)
        .iter()
        .flat_map(|track| &track.clips)
        .any(|clip| clip.look.effects.iter().any(|e| e.other_track().is_some()));
    if kind == TrackKind::Video && reads_tracks {
        return Ok(false);
    }
    let mut moved = false;
    for from in 1..sequence.tracks_of(kind).len() {
        let tracks = sequence.tracks_of(kind);
        if tracks[from].locked {
            continue;
        }
        let alike: Vec<bool> = tracks[..from]
            .iter()
            .map(|to| !to.locked && plays_alike(to, &tracks[from]))
            .collect();
        let movable: Vec<(ClipId, usize)> = tracks[from]
            .clips
            .iter()
            .filter(|clip| !joined(&tracks[from], clip))
            .filter_map(|clip| {
                let span = clip.start..clip.end();
                let lowest = (0..from)
                    .rev()
                    .take_while(|&to| alike[to] && free(&tracks[to], &span))
                    .last()?;
                Some((clip.id, lowest))
            })
            .collect();
        for (id, to) in movable {
            for (_, clip) in ops::remove(sequence, &[id]) {
                ops::insert(sequence, TrackRef { kind, index: to }, clip)?;
                moved = true;
            }
        }
    }
    Ok(moved)
}

/// Whether a transition joins the clip to a neighbour, which moving it alone would undo.
fn joined(track: &Track, clip: &Clip) -> bool {
    clip.transition_in.is_some()
        || clip.transition_out.is_some()
        || track
            .clips
            .iter()
            .any(|next| next.start == clip.end() && next.transition_in.is_some())
}

fn free(track: &Track, span: &Range<Ticks>) -> bool {
    !track
        .clips
        .iter()
        .any(|clip| clip.start < span.end && span.start < clip.end())
}

/// Whether clips are shown or mixed the same on either track.
fn plays_alike(a: &Track, b: &Track) -> bool {
    let settings = |track: &Track| Track {
        clips: Vec::new(),
        name: None,
        sync_unlocked: false,
        ..track.clone()
    };
    settings(a) == settings(b)
}
