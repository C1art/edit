use super::*;
use crate::Param;

#[test]
fn wide_pictures_fit_by_width_and_fill_by_height() {
    let (mut timeline, _) = timeline();
    timeline
        .apply(ImportMedia {
            name: "wide".into(),
            path: "wide.mp4".into(),
            duration: Ticks(1000),
            video: Some(VideoFormat {
                width: 1280,
                height: 720,
                frame_rate: FrameRate::FPS_25,
                ..VideoFormat::default()
            }),
            has_audio: true,
        })
        .unwrap();
    let wide = timeline.sequence().media[1].id;
    timeline
        .apply(place(wide, 0, 0, 100, PlaceMode::Overwrite))
        .unwrap();
    let clips = vec![clip_on(&timeline, V1, 0), clip_on(&timeline, A1, 0)];
    let scale = |timeline: &Timeline| {
        let id = clip_on(timeline, V1, 0);
        timeline
            .sequence()
            .clip(id)
            .unwrap()
            .1
            .look
            .motion
            .scale
            .clone()
    };

    for (fit, hundredths) in [(FrameFit::Fit, 2_500), (FrameFit::Fill, 3_333)] {
        let clips = clips.clone();
        timeline.apply(SetToFrameSize { clips, fit }).unwrap();
        assert_eq!(scale(&timeline), Param::new(hundredths));
    }
}
