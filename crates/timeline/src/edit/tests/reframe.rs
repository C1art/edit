use super::*;
use crate::{AutoReframe, Generator, ReframeMotion, Title, VideoEffect};

fn reframes(timeline: &Timeline, track: TrackRef) -> Vec<ReframeMotion> {
    let sequence = timeline.sequence();
    let clips = &sequence.track(track).unwrap().clips;
    clips
        .iter()
        .flat_map(|clip| &clip.look.effects)
        .filter_map(|effect| match effect {
            VideoEffect::AutoReframe(reframe) => Some(reframe.motion),
            _ => None,
        })
        .collect()
}

#[test]
fn reframing_a_sequence_reshapes_it_and_reframes_only_footage_once() {
    let (mut timeline, footage) = timeline();
    timeline
        .apply(AddGenerated {
            name: "Title".into(),
            generator: Generator::Title(Title::default()),
            placed: None,
        })
        .unwrap();
    let title = timeline.sequence().media[1].id;
    timeline
        .apply(place(footage, 0, 0, 100, PlaceMode::Overwrite))
        .unwrap();
    timeline
        .apply(place(footage, 100, 0, 100, PlaceMode::Overwrite))
        .unwrap();
    timeline
        .apply(PlaceClip {
            video_track: Some(1),
            audio_track: None,
            ..place(title, 0, 0, 100, PlaceMode::Overwrite)
        })
        .unwrap();
    let first = clip_on(&timeline, V1, 0);
    let mut look = timeline.sequence().clip(first).unwrap().1.look.clone();
    look.effects
        .push(VideoEffect::AutoReframe(AutoReframe::default()));
    timeline.apply(SetClipLook { clip: first, look }).unwrap();

    let motion = ReframeMotion::Faster;
    timeline
        .apply(ReframeSequence {
            width: 1080,
            height: 1920,
            motion,
        })
        .unwrap();

    let format = timeline.sequence().format;
    assert_eq!((format.width, format.height), (1080, 1920));
    assert_eq!(reframes(&timeline, V1), [motion, motion]);
    assert_eq!(reframes(&timeline, V2), []);
}
