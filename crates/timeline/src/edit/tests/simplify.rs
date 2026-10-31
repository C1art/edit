use super::*;

fn place_on(source: SourceId, at: i64, video: Option<usize>, audio: Option<usize>) -> PlaceClip {
    PlaceClip {
        video_track: video,
        audio_track: audio,
        ..place(source, at, 0, 200, PlaceMode::Overwrite)
    }
}

#[test]
fn simplifying_drops_unplayed_clips_lowers_the_rest_and_deletes_emptied_tracks() {
    let (mut timeline, source) = timeline();
    for edit in [
        place_on(source, 0, Some(0), Some(0)),
        place_on(source, 500, Some(1), Some(1)),
        place_on(source, 100, Some(1), None),
    ] {
        timeline.apply(edit).unwrap();
    }
    let lowered = clip_on(&timeline, V2, 1);
    let disabled = clip_on(&timeline, V2, 0);
    timeline
        .apply(SetClipsEnabled {
            clips: vec![disabled],
            enabled: false,
        })
        .unwrap();
    // Over the first clip it would pass beneath, so it stays up while it is there.
    let kept_up = place_on(source, 150, Some(1), None);
    timeline.apply(kept_up).unwrap();
    timeline
        .apply(SimplifySequence {
            remove_disabled: true,
            remove_on_tracks_off: false,
            lower_clips: true,
            delete_empty_tracks: true,
        })
        .unwrap();
    let sequence = timeline.sequence();
    assert!(sequence.clip(disabled).is_none());
    assert_eq!(sequence.clip(lowered).unwrap().0, V1);
    assert_eq!((sequence.video.len(), sequence.audio.len()), (2, 1));
    assert_eq!(sequence.audio[0].clips.len(), 2);
}
