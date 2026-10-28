use super::*;
use crate::LinkedSelection;

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

#[test]
fn with_linked_selection_off_a_clip_moves_alone_and_slips_out_of_sync() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 100, PlaceMode::Overwrite))
        .unwrap();
    let (video, audio) = (clip_on(&timeline, V1, 0), clip_on(&timeline, A1, 0));
    timeline
        .apply(SetLinkedSelection(LinkedSelection::Apart))
        .unwrap();
    timeline
        .apply(MoveClips {
            clips: vec![video],
            delta: Ticks(10),
            video_tracks: 0,
            audio_tracks: 0,
        })
        .unwrap();
    assert_eq!(spans(&timeline, V1), [(10, 110, 0)]);
    assert_eq!(spans(&timeline, A1), [(0, 100, 0)]);
    let offset = |id| {
        let sequence = timeline.sequence();
        sequence.sync_offset(sequence.clip(id).unwrap().1)
    };
    assert_eq!(
        (offset(video), offset(audio)),
        (Some(Ticks(10)), Some(Ticks(-10)))
    );
}

#[test]
fn an_unlinked_trim_leaves_the_partner_and_linked_selection_as_they_were() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 100, PlaceMode::Overwrite))
        .unwrap();
    let audio = clip_on(&timeline, A1, 0);
    let trim = Trim {
        clips: vec![audio],
        edge: Edge::Tail,
        delta: Ticks(-20),
        mode: TrimMode::Normal,
    };
    timeline.apply(Unlinked(trim)).unwrap();
    assert_eq!(spans(&timeline, V1), [(0, 100, 0)]);
    assert_eq!(spans(&timeline, A1), [(0, 80, 0)]);
    assert_eq!(
        timeline.sequence().linked_selection,
        LinkedSelection::Together
    );
}

#[test]
fn an_out_of_sync_clip_moves_or_slips_back_into_sync() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 20, 100, PlaceMode::Overwrite))
        .unwrap();
    let (video, audio) = (clip_on(&timeline, V1, 0), clip_on(&timeline, A1, 0));
    timeline
        .apply(SetLinkedSelection(LinkedSelection::Apart))
        .unwrap();
    timeline
        .apply(MoveClips {
            clips: vec![audio],
            delta: Ticks(10),
            video_tracks: 0,
            audio_tracks: 0,
        })
        .unwrap();
    let into_sync = |method| IntoSync {
        clip: audio,
        method,
    };

    timeline.apply(into_sync(SyncMethod::Move)).unwrap();
    assert_eq!(spans(&timeline, A1), [(0, 100, 20)]);
    timeline.undo();
    timeline.apply(into_sync(SyncMethod::Slip)).unwrap();
    assert_eq!(spans(&timeline, A1), [(10, 110, 30)]);
    let sequence = timeline.sequence();
    assert_eq!(sequence.sync_offset(sequence.clip(video).unwrap().1), None);
}
