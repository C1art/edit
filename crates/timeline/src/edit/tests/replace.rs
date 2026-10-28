use super::*;

#[test]
fn replacing_a_clip_moves_it_and_its_link_onto_the_new_media_in_sync() {
    let (mut timeline, first) = timeline();
    timeline
        .apply(ImportMedia {
            name: "b".into(),
            path: "b.mp4".into(),
            duration: Ticks(500),
            video: timeline.sequence().media[0].video,
            has_audio: true,
            still: false,
            interpreted_rate: None,
        })
        .unwrap();
    let second = timeline.sequence().media[1].id;
    timeline
        .apply(place(first, 100, 20, 200, PlaceMode::Overwrite))
        .unwrap();
    let (video, audio) = (clip_on(&timeline, V1, 0), clip_on(&timeline, A1, 0));
    let link = |linked| SetLinked {
        clips: vec![video, audio],
        linked,
    };
    // The sound runs ten ticks ahead of the picture.
    timeline.apply(link(false)).unwrap();
    timeline
        .apply(Slip {
            clips: vec![audio],
            delta: Ticks(10),
        })
        .unwrap();
    timeline.apply(link(true)).unwrap();
    let replace = |source_in| ReplaceClip {
        clip: video,
        source: second,
        source_in: Ticks(source_in),
    };

    assert_eq!(
        timeline.apply(replace(400)),
        Err(TimelineError::NotEnoughSource)
    );
    timeline.apply(replace(50)).unwrap();
    assert_eq!(spans(&timeline, V1), [(100, 300, 50)]);
    assert_eq!(spans(&timeline, A1), [(100, 300, 60)]);
    let sources =
        |timeline: &Timeline, track| timeline.sequence().track(track).unwrap().clips[0].source;
    assert_eq!(
        (sources(&timeline, V1), sources(&timeline, A1)),
        (second, second)
    );
    timeline.undo();
    assert_eq!(sources(&timeline, V1), first);
}
