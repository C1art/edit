use super::*;
use crate::{AudioEffect, EffectPreset, Keyframe, Param, VideoEffect};

#[test]
fn pasted_keyframes_keep_their_distance_from_the_clip_start() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 100, PlaceMode::Overwrite))
        .unwrap();
    timeline
        .apply(place(source, 200, 500, 100, PlaceMode::Overwrite))
        .unwrap();
    let clip = |timeline: &Timeline, track| {
        let id = clip_on(timeline, track, 1);
        timeline.sequence().clip(id).unwrap().1.clone()
    };
    let [mut video, mut audio] = [V1, A1].map(|track| {
        let id = clip_on(&timeline, track, 0);
        timeline.sequence().clip(id).unwrap().1.clone()
    });
    video
        .look
        .motion
        .scale
        .keys
        .push(Keyframe::new(Ticks(10), 5_000));
    video.look.effects.push(VideoEffect::BlackAndWhite);
    audio.volume.keys.push(Keyframe::new(Ticks(20), -600));
    let paste = PasteAttributes {
        video: Some(video),
        audio: Some(audio),
        clips: vec![clip_on(&timeline, V1, 1)],
        attributes: vec![Attribute::Motion, Attribute::Effects, Attribute::Volume],
    };
    timeline.apply(paste.clone()).unwrap();
    timeline.apply(paste).unwrap();

    let pasted = clip(&timeline, V1);
    assert_eq!(pasted.look.motion.scale.keys[0].time, Ticks(510));
    // Effects add to the clip's own rather than replace them.
    assert_eq!(pasted.look.effects.len(), 2);
    assert_eq!(clip(&timeline, A1).volume.keys[0].time, Ticks(520));
}

#[test]
fn an_effect_dropped_on_a_linked_clip_reaches_only_the_track_kind_it_suits() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 100, PlaceMode::Overwrite))
        .unwrap();
    let apply = ApplyEffect {
        clips: vec![clip_on(&timeline, A1, 0)],
        effect: ClipEffect::Video(Box::new(VideoEffect::BlackAndWhite)),
    };
    timeline.apply(apply).unwrap();
    let clip = |track| {
        let id = clip_on(&timeline, track, 0);
        timeline.sequence().clip(id).unwrap().1.clone()
    };
    assert_eq!(clip(V1).look.effects, [VideoEffect::BlackAndWhite]);
    assert!(clip(A1).audio_effects.is_empty());
}

#[test]
fn a_preset_keys_its_effects_as_far_into_the_clip_it_lands_on() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 100, PlaceMode::Overwrite))
        .unwrap();
    timeline
        .apply(place(source, 200, 500, 100, PlaceMode::Overwrite))
        .unwrap();
    let mut blurriness = Param::from_value(0.0);
    blurriness.keys.push(Keyframe::new(Ticks(10), 2_000));
    let keyed = ApplyEffect {
        clips: vec![clip_on(&timeline, V1, 0)],
        effect: ClipEffect::Video(Box::new(VideoEffect::GaussianBlur { blurriness })),
    };
    timeline.apply(keyed).unwrap();
    let gain = Param::from_value(6.0);
    let amplify = ApplyEffect {
        clips: vec![clip_on(&timeline, A1, 0)],
        effect: ClipEffect::Audio(Box::new(AudioEffect::Amplify { gain })),
    };
    timeline.apply(amplify).unwrap();
    let saved = clip_on(&timeline, V1, 0);
    let preset = EffectPreset::from_clip("Blurred".into(), timeline.sequence(), saved).unwrap();

    let apply = ApplyEffect {
        clips: vec![clip_on(&timeline, V1, 1)],
        effect: ClipEffect::Preset(preset),
    };
    timeline.apply(apply).unwrap();
    let clip = |track| {
        let id = clip_on(&timeline, track, 1);
        timeline.sequence().clip(id).unwrap().1.clone()
    };
    let VideoEffect::GaussianBlur { blurriness } = &clip(V1).look.effects[0] else {
        panic!("the preset's blur is missing");
    };
    assert_eq!(blurriness.keys[0].time, Ticks(510));
    assert_eq!(clip(A1).audio_effects.len(), 1);
}
