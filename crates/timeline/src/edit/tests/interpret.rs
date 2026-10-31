use super::*;
use crate::{AlphaInterpretation, ColorSpace, FieldOrder, PixelAspect};

fn interpret(timeline: &mut Timeline, source: SourceId, rate: Option<FrameRate>) {
    timeline
        .apply(InterpretFootage {
            source,
            rate,
            aspect: None,
            alpha: AlphaInterpretation::Straight,
            color: None,
            fields: None,
        })
        .unwrap();
}

#[test]
fn interpreting_slower_keeps_each_clips_frames_until_the_next_clip() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 200, PlaceMode::Overwrite))
        .unwrap();
    timeline
        .apply(place(source, 300, 600, 100, PlaceMode::Overwrite))
        .unwrap();
    interpret(&mut timeline, source, FrameRate::new(25, 2));

    let expected = vec![(0, 300, 0), (300, 500, 600)];
    assert_eq!(spans(&timeline, V1), expected);
    assert_eq!(spans(&timeline, A1), expected);
    let (_, first) = timeline.sequence().clip(clip_on(&timeline, V1, 0)).unwrap();
    assert_eq!(first.source_out(), Ticks(150));
    let (_, second) = timeline.sequence().clip(clip_on(&timeline, V1, 1)).unwrap();
    assert_eq!(second.source_out(), Ticks(700));
    assert_eq!(second.source_time(Ticks(400)), Ticks(650));
}

#[test]
fn interpreted_media_places_as_long_as_its_frames_play() {
    let (mut timeline, source) = timeline();
    interpret(&mut timeline, source, FrameRate::new(50, 1));
    timeline
        .apply(place(source, 0, 100, 400, PlaceMode::Insert))
        .unwrap();
    assert_eq!(spans(&timeline, V1), vec![(0, 200, 100)]);
    let (_, clip) = timeline.sequence().clip(clip_on(&timeline, A1, 0)).unwrap();
    assert_eq!(clip.source_out(), Ticks(500));

    interpret(&mut timeline, source, None);
    assert_eq!(spans(&timeline, V1), vec![(0, 400, 100)]);
}

#[test]
fn reinterpreting_pixels_keeps_clip_lengths_and_the_files_own_shape_clears_it() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 200, PlaceMode::Overwrite))
        .unwrap();
    let anamorphic = PixelAspect::new(4, 3);
    let interpret = |aspect, alpha| InterpretFootage {
        source,
        rate: None,
        aspect,
        alpha,
        color: Some(ColorSpace::Rec2100Hlg),
        fields: Some(FieldOrder::UpperFirst),
    };
    timeline
        .apply(interpret(anamorphic, AlphaInterpretation::Ignore))
        .unwrap();
    let item = &timeline.sequence().media[0];
    assert_eq!(item.pixel_aspect(), anamorphic.unwrap());
    assert_eq!(item.color_space(), ColorSpace::Rec2100Hlg);
    assert_eq!(item.field_order(), FieldOrder::UpperFirst);
    assert_eq!(item.alpha, AlphaInterpretation::Ignore);
    assert_eq!(spans(&timeline, V1), vec![(0, 200, 0)]);

    let own = Some(PixelAspect::SQUARE);
    timeline
        .apply(InterpretFootage {
            color: Some(ColorSpace::Rec709),
            fields: Some(FieldOrder::Progressive),
            ..interpret(own, AlphaInterpretation::Ignore)
        })
        .unwrap();
    let item = &timeline.sequence().media[0];
    assert_eq!(item.interpreted_aspect, None);
    assert_eq!(item.interpreted_color, None);
    assert_eq!(item.interpreted_fields, None);
    assert_eq!(
        timeline.apply(InterpretFootage {
            color: None,
            fields: None,
            ..interpret(None, AlphaInterpretation::Ignore)
        }),
        Err(TimelineError::NothingToEdit)
    );
}

#[test]
fn time_remapping_an_interpreted_clip_keeps_the_frames_it_plays() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 200, PlaceMode::Overwrite))
        .unwrap();
    interpret(&mut timeline, source, FrameRate::new(25, 2));
    let video = clip_on(&timeline, V1, 0);
    let at = |timeline: &Timeline| {
        timeline
            .sequence()
            .clip(video)
            .unwrap()
            .1
            .source_time(Ticks(250))
    };
    let before = at(&timeline);
    timeline
        .apply(AddSpeedKeyframe {
            clip: video,
            at: Ticks(100),
        })
        .unwrap();
    assert_eq!(at(&timeline), before);
    timeline.apply(RemoveTimeRemap { clip: video }).unwrap();
    assert_eq!(at(&timeline), before);
    assert_eq!(
        timeline.sequence().clip(video).unwrap().1.speed,
        crate::Speed::NORMAL
    );
}
