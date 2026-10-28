use super::*;

/// A multi-camera source of the timeline's media and a second camera, placed on V1/A1.
fn multicam() -> (Timeline, SourceId) {
    let (mut timeline, first) = timeline();
    let format = timeline.sequence().format;
    timeline
        .apply(ImportMedia {
            name: "b".into(),
            path: "b.mp4".into(),
            duration: Ticks(800),
            video: Some(format),
            has_audio: true,
        })
        .unwrap();
    let second = timeline.sequence().media[1].id;
    timeline
        .apply(CreateMulticam {
            sources: vec![first, second],
            starts: vec![Ticks(0), Ticks(-300)],
            name: "Cameras".into(),
        })
        .unwrap();
    let cameras = timeline.sequence().media[2].id;
    timeline
        .apply(place(cameras, 0, 0, 600, PlaceMode::Overwrite))
        .unwrap();
    (timeline, cameras)
}

#[test]
fn a_multicam_source_has_a_synced_track_per_camera_and_hears_only_the_first() {
    let (timeline, cameras) = multicam();
    let item = timeline.sequence().media_item(cameras).unwrap();
    let nest = item.nest.as_ref().unwrap();
    assert_eq!((nest.video.len(), nest.audio.len()), (2, 2));
    let muted: Vec<bool> = nest.audio.iter().map(|track| track.muted).collect();
    assert_eq!(muted, [false, true]);
    // The second camera rolled 300 ticks earlier, so the first starts that far in.
    let starts: Vec<i64> = nest
        .video
        .iter()
        .map(|track| track.clips[0].start.0)
        .collect();
    assert_eq!(starts, [300, 0]);
    assert_eq!(item.duration, Ticks(1300));
}

#[test]
fn switching_camera_while_playing_cuts_the_clip_and_its_sound_and_switches_the_rest() {
    let (mut timeline, _) = multicam();
    let clip = clip_on(&timeline, V1, 0);
    let switch = |at| SwitchAngle {
        clip,
        angle: 1,
        at: Some(Ticks(at)),
    };
    timeline.apply(switch(250)).unwrap();
    assert_eq!(spans(&timeline, V1), [(0, 250, 0), (250, 600, 250)]);
    assert_eq!(spans(&timeline, A1), [(0, 250, 0), (250, 600, 250)]);
    let angles: Vec<usize> = timeline.sequence().video[0]
        .clips
        .iter()
        .map(|clip| clip.angle)
        .collect();
    assert_eq!(angles, [0, 1]);
    // Already showing that camera there: nothing to cut.
    let right = clip_on(&timeline, V1, 1);
    let again = SwitchAngle {
        clip: right,
        angle: 1,
        at: Some(Ticks(400)),
    };
    assert_eq!(timeline.apply(again), Err(TimelineError::NothingToEdit));
    assert!(timeline.undo());
    assert_eq!(spans(&timeline, V1), [(0, 600, 0)]);
}
