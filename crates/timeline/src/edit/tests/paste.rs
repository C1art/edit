use super::*;

#[test]
fn paste_overwrites_with_new_ids_and_a_link_of_its_own() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 100, 200, PlaceMode::Overwrite))
        .unwrap();
    timeline
        .apply(place(source, 300, 0, 400, PlaceMode::Overwrite))
        .unwrap();
    let copied = clip_on(&timeline, V1, 0);
    let clipboard = Clipboard::copy(timeline.sequence(), &[copied]).unwrap();
    timeline
        .apply(PasteClips {
            clipboard,
            at: Ticks(400),
            mode: PlaceMode::Overwrite,
            video_offset: 0,
            audio_offset: 0,
        })
        .unwrap();

    let expected = vec![
        (0, 200, 100),
        (300, 400, 0),
        (400, 600, 100),
        (600, 700, 300),
    ];
    assert_eq!(spans(&timeline, V1), expected);
    assert_eq!(spans(&timeline, A1), expected);
    let sequence = timeline.sequence();
    let pasted = clip_on(&timeline, V1, 2);
    assert_ne!(pasted, copied);
    let link = |id| sequence.clip(id).unwrap().1.link;
    assert_eq!(link(pasted), link(clip_on(&timeline, A1, 2)));
    assert_ne!(link(pasted), link(copied));
}

#[test]
fn paste_insert_pushes_everything_later_by_the_copied_span() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 300, PlaceMode::Overwrite))
        .unwrap();
    let clipboard = Clipboard::copy(timeline.sequence(), &[clip_on(&timeline, A1, 0)]).unwrap();
    timeline
        .apply(PasteClips {
            clipboard,
            at: Ticks(100),
            mode: PlaceMode::Insert,
            video_offset: 1,
            audio_offset: 1,
        })
        .unwrap();

    assert_eq!(spans(&timeline, V1), vec![(0, 100, 0), (400, 600, 100)]);
    assert_eq!(spans(&timeline, V2), vec![(100, 400, 0)]);
    assert_eq!(spans(&timeline, A2), vec![(100, 400, 0)]);
}

#[test]
fn paste_onto_a_missing_track_changes_nothing() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 300, PlaceMode::Overwrite))
        .unwrap();
    let before = timeline.sequence().clone();
    let clipboard = Clipboard::copy(timeline.sequence(), &[clip_on(&timeline, V1, 0)]).unwrap();
    let result = timeline.apply(PasteClips {
        clipboard,
        at: Ticks(0),
        mode: PlaceMode::Overwrite,
        video_offset: 2,
        audio_offset: -1,
    });
    assert!(matches!(result, Err(TimelineError::TrackNotFound(_))));
    assert_eq!(timeline.sequence(), &before);
}
