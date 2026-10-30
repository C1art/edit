use super::*;
use crate::{ApplyEffect, Clip, ClipEffect, MotionPreset};

/// A ten-frame clip at 25 fps, 3600 ticks to a frame, from a source long enough for it.
fn ten_frames() -> (Timeline, ClipId) {
    let (mut timeline, _) = timeline();
    timeline
        .apply(ImportMedia {
            name: "long".into(),
            path: "long.mp4".into(),
            duration: Ticks(360_000),
            video: Some(timeline.sequence().format),
            has_audio: false,
            still: false,
            interpreted_rate: None,
            placed: None,
            frame_fit: None,
            timecode: None,
        })
        .unwrap();
    let long = timeline.sequence().media[1].id;
    timeline
        .apply(place(long, 0, 7_200, 36_000, PlaceMode::Overwrite))
        .unwrap();
    let clip = clip_on(&timeline, V1, 0);
    (timeline, clip)
}

fn applied(preset: MotionPreset) -> Clip {
    let (mut timeline, clip) = ten_frames();
    let effect = ClipEffect::Motion(preset);
    timeline
        .apply(ApplyEffect {
            clips: vec![clip],
            effect,
        })
        .unwrap();
    timeline.sequence().clip(clip).unwrap().1.clone()
}

#[test]
fn a_zoom_keys_scale_from_the_first_frame_to_the_last() {
    let keys = applied(MotionPreset::ZoomIn).look.motion.scale.keys;
    let keyed: Vec<_> = keys.iter().map(|key| (key.time, key.hundredths)).collect();
    assert_eq!(keyed, [(Ticks(7_200), 10_000), (Ticks(39_600), 12_000)]);
}

#[test]
fn a_fade_out_shorter_than_half_a_second_takes_half_the_clip() {
    let keys = applied(MotionPreset::FadeOut).look.opacity.keys;
    let keyed: Vec<_> = keys.iter().map(|key| (key.time, key.hundredths)).collect();
    assert_eq!(keyed, [(Ticks(25_200), 10_000), (Ticks(39_600), 0)]);
}
