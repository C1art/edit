use super::*;
use crate::Param;

/// Imports a 1280×720 file, four times the test sequence's width.
fn import_wide(timeline: &mut Timeline, frame_fit: Option<FrameFit>) -> SourceId {
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
            still: false,
            interpreted_rate: None,
            placed: None,
            frame_fit,
            timecode: None,
        })
        .unwrap();
    timeline.sequence().media.last().unwrap().id
}

fn scale(timeline: &Timeline, index: usize) -> Param {
    let id = clip_on(timeline, V1, index);
    let (_, clip) = timeline.sequence().clip(id).unwrap();
    clip.look.motion.scale.clone()
}

#[test]
fn wide_pictures_fit_by_width_and_fill_by_height() {
    let (mut timeline, _) = timeline();
    let wide = import_wide(&mut timeline, None);
    timeline
        .apply(place(wide, 0, 0, 100, PlaceMode::Overwrite))
        .unwrap();
    let clips = vec![clip_on(&timeline, V1, 0), clip_on(&timeline, A1, 0)];

    for (fit, hundredths) in [(FrameFit::Fit, 2_500), (FrameFit::Fill, 3_333)] {
        let clips = clips.clone();
        timeline.apply(SetToFrameSize { clips, fit }).unwrap();
        assert_eq!(scale(&timeline, 0), Param::new(hundredths));
    }
}

#[test]
fn media_imported_with_a_default_scaling_is_placed_at_it() {
    let (mut timeline, _) = timeline();
    let wide = import_wide(&mut timeline, Some(FrameFit::Fill));
    timeline
        .apply(place(wide, 0, 0, 100, PlaceMode::Overwrite))
        .unwrap();
    assert_eq!(scale(&timeline, 0), Param::new(3_333));
    let (_, sound) = timeline.sequence().clip(clip_on(&timeline, A1, 0)).unwrap();
    assert_eq!(sound.look.motion.scale, Param::new(10_000));
}

#[test]
fn blurred_background_fills_a_new_track_beneath_and_undoes_with_it() {
    let (mut timeline, _) = timeline();
    let wide = import_wide(&mut timeline, None);
    timeline
        .apply(place(wide, 0, 0, 100, PlaceMode::Overwrite))
        .unwrap();
    let tracks = timeline.sequence().video.len();
    let original = clip_on(&timeline, V1, 0);
    timeline
        .apply(AddBlurredBackground {
            clips: vec![original],
        })
        .unwrap();

    let sequence = timeline.sequence();
    assert_eq!(sequence.video.len(), tracks + 1);
    let (moved_to, clip) = sequence.clip(original).unwrap();
    assert_eq!(moved_to, TrackRef::video(1));
    let background = &sequence.video[0].clips[0];
    assert_eq!(background.look.motion.scale, Param::new(3_333));
    assert!(clip.group.is_some() && background.group == clip.group);

    timeline.undo();
    assert_eq!(timeline.sequence().video.len(), tracks);
    let (track, _) = timeline.sequence().clip(original).unwrap();
    assert_eq!(track, TrackRef::video(0));
}
