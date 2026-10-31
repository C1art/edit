use super::*;
use crate::{AutomateToSequence, AutomatedPiece, Transition, TransitionKind};

#[test]
fn automating_joins_only_the_cuts_between_placed_pieces_and_undoes_as_one() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 300, PlaceMode::Overwrite))
        .unwrap();
    let before = timeline.sequence().clone();
    let pieces = [300, 500, 700].map(|at| AutomatedPiece {
        source,
        source_in: Ticks(0),
        duration: Ticks(200),
        at: Ticks(at),
    });
    let dissolve = Transition::new(TransitionKind::CrossDissolve, Ticks(500));
    timeline
        .apply(AutomateToSequence {
            pieces: pieces.to_vec(),
            video_track: Some(0),
            audio_track: Some(0),
            mode: PlaceMode::Overwrite,
            video_transition: Some(dissolve),
            audio_transition: Some(dissolve),
        })
        .unwrap();
    for track in [V1, A1] {
        let ins: Vec<_> = timeline
            .sequence()
            .track(track)
            .unwrap()
            .clips
            .iter()
            .map(|clip| clip.transition_in.map(|t| t.duration.0))
            .collect();
        // The cut onto the clip already there is left alone.
        assert_eq!(ins, [None, None, Some(200), Some(200)]);
    }
    timeline.undo();
    assert_eq!(timeline.sequence(), &before);
}
