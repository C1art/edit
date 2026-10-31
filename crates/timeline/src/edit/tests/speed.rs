use super::*;
use crate::Speed;

fn speeds(timeline: &Timeline, track: TrackRef) -> Vec<Speed> {
    let track = timeline.sequence().track(track).unwrap();
    track.clips.iter().map(|clip| clip.speed).collect()
}

#[test]
fn rippled_speed_change_keeps_the_source_range_and_moves_what_follows() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 100, 400, PlaceMode::Overwrite))
        .unwrap();
    timeline
        .apply(place(source, 400, 0, 100, PlaceMode::Overwrite))
        .unwrap();
    let clip = clip_on(&timeline, V1, 0);
    let double = Speed::new(2, 1).unwrap();
    timeline
        .apply(SetClipSpeed {
            clips: vec![clip],
            speed: double,
            ripple: true,
            reverse: false,
            maintain_pitch: false,
        })
        .unwrap();

    let expected = vec![(0, 200, 100), (200, 300, 0)];
    assert_eq!(spans(&timeline, V1), expected);
    assert_eq!(spans(&timeline, A1), expected);
    let (_, slowed) = timeline.sequence().clip(clip).unwrap();
    assert_eq!(slowed.source_out(), Ticks(500));
}

#[test]
fn slowing_down_without_ripple_stops_at_the_next_clip() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 200, PlaceMode::Overwrite))
        .unwrap();
    timeline
        .apply(place(source, 300, 0, 100, PlaceMode::Overwrite))
        .unwrap();
    timeline
        .apply(SetClipSpeed {
            clips: vec![clip_on(&timeline, V1, 0)],
            speed: Speed::new(1, 2).unwrap(),
            ripple: false,
            reverse: false,
            maintain_pitch: false,
        })
        .unwrap();
    assert_eq!(spans(&timeline, V1), vec![(0, 300, 0), (300, 400, 0)]);
    let (_, slowed) = timeline.sequence().clip(clip_on(&timeline, A1, 0)).unwrap();
    assert_eq!(slowed.source_out(), Ticks(150));
}

#[test]
fn trims_and_cuts_at_double_speed_advance_the_source_twice_as_far() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 400, PlaceMode::Overwrite))
        .unwrap();
    let clip = clip_on(&timeline, V1, 0);
    timeline
        .apply(SetClipSpeed {
            clips: vec![clip],
            speed: Speed::new(2, 1).unwrap(),
            ripple: false,
            reverse: false,
            maintain_pitch: false,
        })
        .unwrap();
    timeline
        .apply(Trim {
            clips: vec![clip],
            edge: Edge::Head,
            delta: Ticks(50),
            mode: TrimMode::Normal,
        })
        .unwrap();
    timeline
        .apply(Razor {
            at: Ticks(150),
            target: RazorTarget::Tracks(vec![V1, A1]),
        })
        .unwrap();
    assert_eq!(spans(&timeline, V1), vec![(50, 150, 100), (150, 200, 300)]);
    let (_, tail) = timeline.sequence().clip(clip_on(&timeline, V1, 1)).unwrap();
    assert_eq!(tail.source_out(), Ticks(400));
}

#[test]
fn rate_stretch_fits_the_same_source_into_the_new_length() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 100, 0, 300, PlaceMode::Overwrite))
        .unwrap();
    timeline
        .apply(RateStretch {
            clips: vec![clip_on(&timeline, A1, 0)],
            edge: Edge::Head,
            delta: Ticks(-100),
        })
        .unwrap();
    assert_eq!(spans(&timeline, V1), vec![(0, 400, 0)]);
    assert_eq!(speeds(&timeline, V1), vec![Speed::new(3, 4).unwrap()]);
    let (_, stretched) = timeline.sequence().clip(clip_on(&timeline, V1, 0)).unwrap();
    assert_eq!(stretched.source_out(), Ticks(300));
}

#[test]
fn reversed_clips_play_their_range_backwards_through_trims_and_cuts() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 100, 400, PlaceMode::Overwrite))
        .unwrap();
    let clip = clip_on(&timeline, V1, 0);
    timeline
        .apply(SetClipSpeed {
            clips: vec![clip],
            speed: Speed::NORMAL,
            ripple: false,
            reverse: true,
            maintain_pitch: false,
        })
        .unwrap();
    let source_at = |timeline: &Timeline, index, position| {
        let id = clip_on(timeline, V1, index);
        timeline
            .sequence()
            .clip(id)
            .unwrap()
            .1
            .source_time(Ticks(position))
    };
    assert_eq!(source_at(&timeline, 0, 0), Ticks(499));
    assert_eq!(source_at(&timeline, 0, 399), Ticks(100));

    // Trimming the head drops the end of the source; the tail still shows its start.
    timeline
        .apply(Trim {
            clips: vec![clip],
            edge: Edge::Head,
            delta: Ticks(100),
            mode: TrimMode::Normal,
        })
        .unwrap();
    assert_eq!(spans(&timeline, V1), vec![(100, 400, 100)]);
    assert_eq!(source_at(&timeline, 0, 100), Ticks(399));

    timeline
        .apply(Razor {
            at: Ticks(200),
            target: RazorTarget::Tracks(vec![V1, A1]),
        })
        .unwrap();
    assert_eq!(spans(&timeline, V1), vec![(100, 200, 300), (200, 400, 100)]);
    assert_eq!(source_at(&timeline, 0, 199), Ticks(300));
    assert_eq!(source_at(&timeline, 1, 200), Ticks(299));
}

#[test]
fn fit_to_fill_changes_speed_so_the_source_range_fills_the_sequence_range() {
    let (mut timeline, source) = timeline();
    let fit = PlaceClip {
        fill: Some(Ticks(400)),
        ..place(source, 100, 50, 200, PlaceMode::Overwrite)
    };
    timeline.apply(fit).unwrap();
    for track in [V1, A1] {
        let (_, clip) = timeline
            .sequence()
            .clip(clip_on(&timeline, track, 0))
            .unwrap();
        assert_eq!((clip.start, clip.duration), (Ticks(100), Ticks(400)));
        assert_eq!((clip.source_in, clip.source_out()), (Ticks(50), Ticks(250)));
        assert_eq!(clip.speed, Speed::new(1, 2).unwrap());
    }
}

#[test]
fn rippled_speed_change_of_neighbours_moves_each_by_its_own_change() {
    let (mut timeline, source) = timeline();
    for start in [0, 100, 200] {
        timeline
            .apply(place(source, start, 0, 100, PlaceMode::Overwrite))
            .unwrap();
    }
    timeline
        .apply(SetClipSpeed {
            clips: vec![clip_on(&timeline, V1, 0), clip_on(&timeline, V1, 1)],
            speed: Speed::new(1, 2).unwrap(),
            ripple: true,
            reverse: false,
            maintain_pitch: false,
        })
        .unwrap();
    let expected = vec![(0, 200, 0), (200, 400, 0), (400, 500, 0)];
    assert_eq!(spans(&timeline, V1), expected);
    assert_eq!(spans(&timeline, A1), expected);
}
