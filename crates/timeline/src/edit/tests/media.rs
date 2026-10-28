use super::*;

fn relink(source: SourceId, path: &str, duration: i64) -> RelinkMedia {
    RelinkMedia {
        source,
        media: ImportMedia {
            name: path.into(),
            path: path.into(),
            duration: Ticks(duration),
            video: None,
            has_audio: true,
            still: false,
        },
    }
}

#[test]
fn relinking_follows_the_new_file_unless_renamed_and_needs_room_for_the_clips() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 500, 300, PlaceMode::Overwrite))
        .unwrap();
    timeline
        .apply(SetProxy {
            source,
            proxy: Some("a_proxy.mp4".into()),
        })
        .unwrap();
    assert_eq!(
        timeline.apply(relink(source, "short.wav", 700)),
        Err(TimelineError::OutsideMedia(clip_on(&timeline, V1, 0)))
    );
    // "a" differs from the file's name, so it was chosen and survives.
    timeline.apply(relink(source, "b.wav", 800)).unwrap();
    let item = &timeline.sequence().media[0];
    assert_eq!((item.name.as_str(), item.proxy.as_ref()), ("a", None));
    timeline
        .apply(RenameMedia {
            source,
            name: "b.wav".into(),
        })
        .unwrap();
    timeline.apply(relink(source, "c.wav", 900)).unwrap();
    let item = &timeline.sequence().media[0];
    assert_eq!((item.name.as_str(), item.duration), ("c.wav", Ticks(900)));
}

#[test]
fn a_subclip_places_its_part_of_the_file_and_cannot_reach_past_it() {
    let (mut timeline, source) = timeline();
    let subclip = |start, end| MakeSubclip {
        source,
        range: Ticks(start)..Ticks(end),
        name: "take 2".into(),
    };
    // The file is 1000 ticks long.
    timeline.apply(subclip(200, 5_000)).unwrap();
    let item = timeline.sequence().media.last().unwrap();
    assert_eq!(item.name, "take 2");
    assert_eq!(
        item.default_range(Timebase::HZ_90K),
        Ticks(200)..Ticks(1000)
    );
    assert_eq!(
        timeline.apply(subclip(1000, 1010)),
        Err(TimelineError::NonPositiveDuration)
    );
}

#[test]
fn a_duplicate_follows_its_original_under_a_new_id_and_undoes() {
    let (mut timeline, source) = timeline();
    timeline.apply(DuplicateMedia { source }).unwrap();
    let media = &timeline.sequence().media;
    assert_eq!(media.len(), 2);
    assert_eq!(media[1].name, format!("{} Copy", media[0].name));
    assert_ne!(media[1].id, source);
    timeline.undo();
    assert_eq!(timeline.sequence().media.len(), 1);
}

#[test]
fn media_made_offline_comes_back_by_relinking_even_to_the_same_file() {
    let (mut timeline, source) = timeline();
    let offline = || MakeOffline {
        sources: vec![source],
    };
    timeline.apply(offline()).unwrap();
    assert!(timeline.sequence().media[0].offline);
    assert_eq!(timeline.apply(offline()), Err(TimelineError::NothingToEdit));
    timeline.apply(relink(source, "a.mp4", 1000)).unwrap();
    assert!(!timeline.sequence().media[0].offline);
    assert_eq!(
        timeline.apply(relink(source, "a.mp4", 1000)),
        Err(TimelineError::NothingToEdit)
    );
}

#[test]
fn a_still_places_its_usual_length_but_holds_for_as_long_as_a_clip_runs() {
    let (mut timeline, _) = timeline();
    let timebase = Timebase::HZ_90K;
    timeline
        .apply(ImportMedia {
            name: "logo.png".into(),
            path: "logo.png".into(),
            duration: crate::held_length(timebase),
            video: Some(VideoFormat::default()),
            has_audio: false,
            still: true,
        })
        .unwrap();
    let still = timeline.sequence().media.last().unwrap().clone();
    assert_eq!(still.default_range(timebase), Ticks::ZERO..Ticks(450_000));
    timeline
        .apply(place(still.id, 0, 0, 900_000, PlaceMode::Overwrite))
        .unwrap();
    assert_eq!(spans(&timeline, V1), [(0, 900_000, 0)]);
}
