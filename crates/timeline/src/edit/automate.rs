use std::collections::HashSet;

use crate::edit::ops;
use crate::{
    ClipId, Edit, PlaceClip, PlaceMode, Sequence, SourceId, Ticks, TimelineError, TrackKind,
    Transition,
};

/// A range of media and where it goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutomatedPiece {
    pub source: SourceId,
    pub source_in: Ticks,
    pub duration: Ticks,
    pub at: Ticks,
}

/// Lays several ranges of media into the sequence as one edit, as Automate to Sequence
/// does, with the given transitions put at the cuts where two of them meet.
/// Pieces with nothing to place on the given tracks are passed over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutomateToSequence {
    pub pieces: Vec<AutomatedPiece>,
    pub video_track: Option<usize>,
    pub audio_track: Option<usize>,
    pub mode: PlaceMode,
    pub video_transition: Option<Transition>,
    pub audio_transition: Option<Transition>,
}

impl Edit for AutomateToSequence {
    fn name(&self) -> &'static str {
        "Automate to Sequence"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let existing: HashSet<ClipId> = sequence
            .tracks()
            .flat_map(|(_, track)| track.clips.iter().map(|clip| clip.id))
            .collect();
        let mut placed = 0;
        for piece in &self.pieces {
            let place = PlaceClip {
                source: piece.source,
                source_in: piece.source_in,
                duration: piece.duration,
                at: piece.at,
                video_track: self.video_track,
                audio_track: self.audio_track,
                mode: self.mode,
                fill: None,
            };
            match place.edit(sequence) {
                Ok(()) => placed += 1,
                Err(TimelineError::NothingToEdit) => {}
                Err(error) => return Err(error),
            }
        }
        if placed == 0 {
            return Err(TimelineError::NothingToEdit);
        }
        join_new_clips(
            sequence,
            &existing,
            [self.video_transition, self.audio_transition],
        )
    }
}

/// Puts a transition at each cut between two clips that weren't among `existing`, no
/// longer than the shorter of the two.
fn join_new_clips(
    sequence: &mut Sequence,
    existing: &HashSet<ClipId>,
    [video, audio]: [Option<Transition>; 2],
) -> Result<(), TimelineError> {
    let mut cuts = Vec::new();
    for (track_ref, track) in sequence.tracks().filter(|(_, track)| !track.locked) {
        let transition = match track_ref.kind {
            TrackKind::Video => video,
            TrackKind::Audio => audio,
        };
        let Some(transition) = transition else {
            continue;
        };
        let new = |id: &ClipId| !existing.contains(id);
        for pair in track.clips.windows(2) {
            let [before, after] = pair else {
                continue;
            };
            if new(&before.id) && new(&after.id) && before.end() == after.start {
                let shorter = before.duration.min(after.duration);
                cuts.push((after.id, transition.duration.min(shorter), transition));
            }
        }
    }
    for (id, duration, transition) in cuts {
        ops::clip_mut(sequence, id)?.transition_in = Some(Transition {
            duration,
            ..transition
        });
    }
    Ok(())
}
