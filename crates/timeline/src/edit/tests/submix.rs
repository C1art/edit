use super::*;

#[test]
fn deleting_a_submix_sends_its_tracks_to_the_master_and_undo_restores_the_route() {
    let (mut timeline, _) = timeline();
    timeline.apply(AddSubmix).unwrap();
    timeline.apply(AddSubmix).unwrap();
    let [first, second] = [0, 1].map(|index| timeline.sequence().submixes[index].id);
    timeline
        .apply(SetTrackOutput {
            track: A1,
            output: Some(first),
        })
        .unwrap();
    timeline
        .apply(SetTrackOutput {
            track: A2,
            output: Some(second),
        })
        .unwrap();
    timeline.apply(RemoveSubmix { submix: first }).unwrap();

    let outputs = |timeline: &Timeline| -> Vec<_> {
        timeline
            .sequence()
            .audio
            .iter()
            .map(|track| track.output)
            .collect()
    };
    assert_eq!(outputs(&timeline), [None, Some(second)]);
    let unknown = SetTrackOutput {
        track: A2,
        output: Some(first),
    };
    assert_eq!(timeline.apply(unknown), Err(TimelineError::NothingToEdit));
    timeline.undo();
    assert_eq!(outputs(&timeline), [Some(first), Some(second)]);
}
