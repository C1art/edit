use super::*;

fn disabled(timeline: &Timeline, track: TrackRef) -> Vec<bool> {
    let track = timeline.sequence().track(track).unwrap();
    track.clips.iter().map(|clip| clip.disabled).collect()
}

#[test]
fn disabling_a_clip_takes_its_linked_partner_along_and_undoes_together() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 100, PlaceMode::Overwrite))
        .unwrap();
    timeline
        .apply(place(source, 100, 0, 100, PlaceMode::Overwrite))
        .unwrap();
    let first = clip_on(&timeline, V1, 0);
    let disable = SetClipsEnabled {
        clips: vec![first],
        enabled: false,
    };
    timeline.apply(disable.clone()).unwrap();
    assert_eq!(disabled(&timeline, V1), [true, false]);
    assert_eq!(disabled(&timeline, A1), [true, false]);
    assert_eq!(timeline.apply(disable), Err(TimelineError::NothingToEdit));

    timeline.undo();
    assert_eq!(disabled(&timeline, A1), [false, false]);
}
