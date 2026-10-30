use super::*;
use crate::{AdjustClips, Param};

#[test]
fn adjusting_several_clips_is_one_step_that_keeps_their_animation() {
    let (mut timeline, source) = timeline();
    for at in [0, 100] {
        timeline
            .apply(place(source, at, 0, 100, PlaceMode::Overwrite))
            .unwrap();
    }
    let (first, second) = (clip_on(&timeline, V1, 0), clip_on(&timeline, V1, 1));
    let sound = clip_on(&timeline, A1, 0);
    let sequence = timeline.sequence();
    let mut animated = sequence.clip(second).unwrap().1.look.clone();
    animated.opacity.toggle_animation(Ticks(0));
    animated.opacity.set_at(Ticks(50), 50.0);
    let mut looks = vec![
        (first, sequence.clip(first).unwrap().1.look.clone()),
        (second, animated),
    ];
    for (_, look) in &mut looks {
        look.opacity.offset(-20.0, 0.0..=100.0);
    }
    let mut volume = Param::from_value(0.0);
    volume.offset(-6.0, -96.0..=15.0);
    timeline
        .apply(AdjustClips {
            looks,
            volumes: vec![(sound, volume)],
        })
        .unwrap();

    let sequence = timeline.sequence();
    let opacity = |id| sequence.clip(id).unwrap().1.look.opacity.clone();
    assert_eq!(opacity(first).value(), 80.0);
    let keys: Vec<f32> = opacity(second).keys.iter().map(|key| key.value()).collect();
    assert_eq!(keys, [80.0, 30.0]);
    assert_eq!(sequence.clip(sound).unwrap().1.volume.value(), -6.0);

    assert!(timeline.undo());
    let sequence = timeline.sequence();
    assert_eq!(sequence.clip(first).unwrap().1.look.opacity.value(), 100.0);
    assert_eq!(sequence.clip(sound).unwrap().1.volume.value(), 0.0);
}
