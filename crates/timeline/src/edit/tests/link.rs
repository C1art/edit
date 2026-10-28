use super::*;

#[test]
fn unlinking_frees_partners_and_linking_joins_whole_groups() {
    let (mut timeline, source) = timeline();
    for at in [0, 100] {
        timeline
            .apply(place(source, at, 0, 100, PlaceMode::Overwrite))
            .unwrap();
    }
    let [video, audio, later] =
        [(V1, 0), (A1, 0), (V1, 1)].map(|(track, index)| clip_on(&timeline, track, index));
    timeline
        .apply(SetLinked {
            clips: vec![video],
            linked: false,
        })
        .unwrap();
    assert_eq!(timeline.sequence().with_links(&[audio]), [audio]);

    let join = SetLinked {
        clips: vec![video, later],
        linked: true,
    };
    timeline.apply(join.clone()).unwrap();
    let mut joined = vec![video, later, clip_on(&timeline, A1, 1)];
    joined.sort_unstable();
    assert_eq!(timeline.sequence().with_links(&[video]), joined);
    assert_eq!(timeline.apply(join), Err(TimelineError::NothingToEdit));
}
