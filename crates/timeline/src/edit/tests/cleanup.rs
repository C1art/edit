use super::*;

fn import(timeline: &mut Timeline, name: &str, path: &str) -> SourceId {
    timeline
        .apply(ImportMedia {
            name: name.into(),
            path: path.into(),
            duration: Ticks(1000),
            video: timeline.sequence().media[0].video,
            has_audio: true,
            still: false,
            interpreted_rate: None,
            placed: None,
            frame_fit: None,
            timecode: None,
        })
        .unwrap();
    timeline.sequence().media.last().unwrap().id
}

#[test]
fn consolidating_merges_the_same_file_into_the_first_item_and_moves_its_clips() {
    let (mut timeline, source) = timeline();
    let copy = import(&mut timeline, "a again", "a.mp4");
    let other = import(&mut timeline, "b", "b.mp4");
    timeline
        .apply(place(copy, 0, 0, 400, PlaceMode::Overwrite))
        .unwrap();
    timeline.apply(ConsolidateDuplicates).unwrap();
    let ids: Vec<_> = timeline
        .sequence()
        .media
        .iter()
        .map(|item| item.id)
        .collect();
    assert_eq!(ids, [source, other]);
    let clip = clip_on(&timeline, V1, 0);
    assert_eq!(timeline.sequence().clip(clip).unwrap().1.source, source);
    assert_eq!(
        timeline.apply(ConsolidateDuplicates),
        Err(TimelineError::NothingToEdit)
    );
}

#[test]
fn removing_unused_keeps_items_clips_use_and_sequences() {
    let (mut timeline, source) = timeline();
    import(&mut timeline, "b", "b.mp4");
    timeline
        .apply(place(source, 0, 0, 400, PlaceMode::Overwrite))
        .unwrap();
    timeline
        .apply(AddNest {
            name: "Sequence 02".into(),
        })
        .unwrap();
    let before = timeline.sequence().media.len();
    timeline.apply(RemoveUnused).unwrap();
    let media = &timeline.sequence().media;
    assert_eq!(media.len(), before - 1);
    assert!(media.iter().all(|item| item.path.as_os_str() != "b.mp4"));
}
