use super::*;

#[test]
fn nesting_moves_clips_into_a_sequence_that_edits_and_undoes_in_place() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 300, PlaceMode::Overwrite))
        .unwrap();
    let video_only = |at, track| PlaceClip {
        video_track: Some(track),
        audio_track: None,
        ..place(source, at, 0, 100, PlaceMode::Overwrite)
    };
    timeline.apply(video_only(400, 1)).unwrap();
    let clips = vec![clip_on(&timeline, V1, 0), clip_on(&timeline, V2, 0)];
    timeline
        .apply(NestClips {
            clips,
            name: "Nest".into(),
        })
        .unwrap();

    assert_eq!(spans(&timeline, V1), [(0, 500, 0)]);
    assert_eq!(spans(&timeline, A1), [(0, 500, 0)]);
    assert!(spans(&timeline, V2).is_empty());
    let nest = timeline.sequence().media[1].id;
    assert!(timeline.set_open_nest(Some(nest)));
    assert_eq!(spans(&timeline, V1), [(0, 300, 0)]);
    assert_eq!(spans(&timeline, V2), [(400, 500, 0)]);
    assert_eq!(spans(&timeline, A1), [(0, 300, 0)]);

    // Edits act on the open nest, which grows to hold them.
    timeline.apply(video_only(600, 0)).unwrap();
    assert_eq!(
        timeline.project().media_item(nest).unwrap().duration,
        Ticks(700)
    );
    let into_itself = PlaceClip {
        audio_track: None,
        ..place(nest, 800, 0, 100, PlaceMode::Overwrite)
    };
    assert_eq!(
        timeline.apply(into_itself),
        Err(TimelineError::NestCycle(nest))
    );

    timeline.undo();
    timeline.undo();
    assert_eq!(timeline.open_nest(), None);
    assert_eq!(spans(&timeline, V1), [(0, 300, 0)]);
    assert_eq!(spans(&timeline, V2), [(400, 500, 0)]);
}
