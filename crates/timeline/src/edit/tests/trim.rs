use super::*;

#[test]
fn normal_trim_cannot_run_into_the_next_clip_and_clamp_stops_at_it() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 100, PlaceMode::Overwrite))
        .unwrap();
    timeline
        .apply(place(source, 150, 0, 100, PlaceMode::Overwrite))
        .unwrap();
    let first = clip_on(&timeline, V1, 0);
    let trim = |delta| Trim {
        clips: vec![first],
        edge: Edge::Tail,
        delta,
        mode: TrimMode::Normal,
    };

    assert!(matches!(
        timeline.apply(trim(Ticks(60))),
        Err(TimelineError::ClipOverlap { .. })
    ));
    assert_eq!(clamp_delta(timeline.sequence(), Ticks(60), trim), Ticks(50));
}

#[test]
fn delta_bounds_clamp_like_clamp_delta_and_stop_trying_once_settled() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 100, PlaceMode::Overwrite))
        .unwrap();
    timeline
        .apply(place(source, 150, 0, 100, PlaceMode::Overwrite))
        .unwrap();
    let first = clip_on(&timeline, V1, 0);
    let tries = std::cell::Cell::new(0);
    let trim = |delta| {
        tries.set(tries.get() + 1);
        Trim {
            clips: vec![first],
            edge: Edge::Tail,
            delta,
            mode: TrimMode::Normal,
        }
    };
    let mut bounds = DeltaBounds::default();
    let sequence = timeline.sequence();

    assert_eq!(bounds.clamp(sequence, Ticks(60), trim), Ticks(50));
    assert_eq!(bounds.clamp(sequence, Ticks(-30), trim), Ticks(-30));
    let settled = tries.get();
    assert_eq!(bounds.clamp(sequence, Ticks(80), trim), Ticks(50));
    assert_eq!(bounds.clamp(sequence, Ticks(40), trim), Ticks(40));
    assert_eq!(bounds.clamp(sequence, Ticks(-10), trim), Ticks(-10));
    assert_eq!(tries.get(), settled);
}

#[test]
fn ripple_tail_trim_moves_later_clips_on_all_tracks_and_rejects_collisions() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 100, PlaceMode::Overwrite))
        .unwrap();
    timeline
        .apply(place(source, 100, 0, 100, PlaceMode::Overwrite))
        .unwrap();
    let mut on_a2 = place(source, 150, 0, 100, PlaceMode::Overwrite);
    on_a2.video_track = None;
    on_a2.audio_track = Some(1);
    timeline.apply(on_a2).unwrap();
    let first = clip_on(&timeline, V1, 0);
    let ripple = |delta| Trim {
        clips: vec![first],
        edge: Edge::Tail,
        delta,
        mode: TrimMode::Ripple,
    };

    timeline.apply(ripple(Ticks(30))).unwrap();
    assert_eq!(spans(&timeline, V1), [(0, 130, 0), (130, 230, 0)]);
    assert_eq!(spans(&timeline, A2), [(180, 280, 0)]);

    timeline.apply(ripple(Ticks(-50))).unwrap();
    assert_eq!(spans(&timeline, A2), [(130, 230, 0)]);
}

#[test]
fn ripple_head_trim_keeps_the_clip_start_and_pulls_the_rest_in() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 100, PlaceMode::Overwrite))
        .unwrap();
    timeline
        .apply(place(source, 100, 0, 100, PlaceMode::Overwrite))
        .unwrap();
    let second = clip_on(&timeline, V1, 1);
    timeline
        .apply(Trim {
            clips: vec![second],
            edge: Edge::Head,
            delta: Ticks(40),
            mode: TrimMode::Ripple,
        })
        .unwrap();
    assert_eq!(spans(&timeline, V1), [(0, 100, 0), (100, 160, 40)]);
}

#[test]
fn roll_moves_the_cut_without_moving_anything_else() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 100, PlaceMode::Overwrite))
        .unwrap();
    timeline
        .apply(place(source, 100, 300, 100, PlaceMode::Overwrite))
        .unwrap();
    timeline
        .apply(Roll {
            at: Ticks(100),
            tracks: vec![V1, A1],
            delta: Ticks(-20),
        })
        .unwrap();
    assert_eq!(spans(&timeline, A1), [(0, 80, 0), (80, 200, 280)]);
}

#[test]
fn slip_is_bounded_by_the_media() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 100, 800, PlaceMode::Overwrite))
        .unwrap();
    let clip = clip_on(&timeline, V1, 0);
    let slip = |delta| Slip {
        clips: vec![clip],
        delta,
    };
    assert_eq!(
        clamp_delta(timeline.sequence(), Ticks(500), slip),
        Ticks(100)
    );
    assert_eq!(
        clamp_delta(timeline.sequence(), Ticks(-500), slip),
        Ticks(-100)
    );
}

#[test]
fn slide_trades_length_between_the_neighbors() {
    let (mut timeline, source) = timeline();
    for (at, source_in) in [(0, 0), (100, 300), (200, 600)] {
        timeline
            .apply(place(source, at, source_in, 100, PlaceMode::Overwrite))
            .unwrap();
    }
    let middle = clip_on(&timeline, V1, 1);
    timeline
        .apply(Slide {
            clips: vec![middle],
            delta: Ticks(30),
        })
        .unwrap();
    assert_eq!(
        spans(&timeline, V1),
        [(0, 130, 0), (130, 230, 300), (230, 300, 630)]
    );
}
