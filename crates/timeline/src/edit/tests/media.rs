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
