//! Several tracks added at once, anywhere among those there are.

use std::sync::Arc;

use crate::{Edit, Sequence, TimelineError, Track, TrackKind};

/// Adds empty video and audio tracks in one step; effects keep reading the video tracks
/// they read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AddTracks {
    pub video: NewTracks,
    pub audio: NewTracks,
}

/// `count` tracks, the first at index `at`, moving those from there up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NewTracks {
    pub at: usize,
    pub count: usize,
}

impl Edit for AddTracks {
    fn name(&self) -> &'static str {
        "Add Tracks"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        if self.video.count + self.audio.count == 0 {
            return Err(TimelineError::NothingToEdit);
        }
        add(sequence, TrackKind::Audio, self.audio);
        let count = self.video.count;
        let at = add(sequence, TrackKind::Video, self.video);
        if count > 0 {
            renumber_layers(sequence, |layer| {
                Some(if layer >= at { layer + count } else { layer })
            });
        }
        Ok(())
    }
}

/// Where the first of `new` went.
fn add(sequence: &mut Sequence, kind: TrackKind, new: NewTracks) -> usize {
    let tracks = sequence.tracks_of_mut(kind);
    let at = new.at.min(tracks.len());
    let added = std::iter::repeat_with(Track::default).take(new.count);
    tracks.splice(at..at, added);
    at
}

/// Points effects that read another video track at where that track went, or at none.
pub(super) fn renumber_layers(sequence: &mut Sequence, moved: impl Fn(usize) -> Option<usize>) {
    for (_, track) in sequence.tracks_mut() {
        for clip in &mut track.clips {
            let reads_moved = clip.look.effects.iter().any(|effect| {
                effect
                    .other_track()
                    .is_some_and(|layer| moved(layer) != Some(layer))
            });
            if !reads_moved {
                continue;
            }
            for effect in &mut Arc::make_mut(clip).look.effects {
                if let Some(layer) = effect.other_track_mut() {
                    *layer = layer.and_then(&moved);
                }
            }
        }
    }
}
