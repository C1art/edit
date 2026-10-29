use super::*;
use crate::Track;

#[test]
fn merging_a_camera_with_a_recorder_links_the_picture_to_the_recorded_sound_in_sync() {
    let (mut timeline, camera) = timeline();
    timeline
        .apply(ImportMedia {
            name: "recorder".into(),
            path: "recorder.wav".into(),
            duration: Ticks(1200),
            video: None,
            has_audio: true,
            still: false,
            interpreted_rate: None,
        })
        .unwrap();
    let recorder = timeline.sequence().media[1].id;
    timeline
        .apply(MergeClips {
            sources: vec![recorder, camera],
            starts: vec![Ticks(-200), Ticks(0)],
            remove_camera_audio: true,
            name: "a Merged".into(),
        })
        .unwrap();
    let item = &timeline.sequence().media[2];
    let nest = item.nest.as_ref().unwrap();
    let only_clip = |tracks: &[Track]| {
        assert_eq!(tracks.len(), 1);
        tracks[0].clips[0].clone()
    };
    let picture = only_clip(&nest.video);
    let sound = only_clip(&nest.audio);
    // The recorder rolled 200 ticks before the camera.
    assert_eq!((picture.source, picture.start), (camera, Ticks(200)));
    assert_eq!((sound.source, sound.start), (recorder, Ticks(0)));
    assert!(picture.link.is_some() && picture.link == sound.link);
    assert_eq!(item.duration, Ticks(1200));
}
