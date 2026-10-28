use super::*;
use crate::{
    ClipId, FrameRate, Sequence, SourceId, Ticks, Timebase, Timeline, TimelineError, TrackRef,
    VideoFormat,
};

const V1: TrackRef = TrackRef::video(0);
const V2: TrackRef = TrackRef::video(1);
const A1: TrackRef = TrackRef::audio(0);
const A2: TrackRef = TrackRef::audio(1);

/// A timeline with two tracks of each kind and one 1000-tick media item with video
/// and audio.
fn timeline() -> (Timeline, SourceId) {
    let format = VideoFormat {
        width: 320,
        height: 240,
        frame_rate: FrameRate::FPS_25,
    };
    let mut sequence = Sequence::new(Timebase::HZ_90K, format);
    sequence.video.resize_with(2, Default::default);
    sequence.audio.resize_with(2, Default::default);
    let mut timeline = Timeline::new(sequence);
    timeline
        .apply(ImportMedia {
            name: "a".into(),
            path: "a.mp4".into(),
            duration: Ticks(1000),
            video: Some(format),
            has_audio: true,
        })
        .unwrap();
    let source = timeline.sequence().media[0].id;
    (timeline, source)
}

fn place(source: SourceId, at: i64, source_in: i64, duration: i64, mode: PlaceMode) -> PlaceClip {
    PlaceClip {
        source,
        source_in: Ticks(source_in),
        duration: Ticks(duration),
        at: Ticks(at),
        video_track: Some(0),
        audio_track: Some(0),
        mode,
    }
}

/// `(start, end, source_in)` of each clip on the track.
fn spans(timeline: &Timeline, track: TrackRef) -> Vec<(i64, i64, i64)> {
    timeline
        .sequence()
        .track(track)
        .unwrap()
        .clips
        .iter()
        .map(|clip| (clip.start.0, clip.end().0, clip.source_in.0))
        .collect()
}

fn clip_on(timeline: &Timeline, track: TrackRef, index: usize) -> ClipId {
    timeline.sequence().track(track).unwrap().clips[index].id
}

#[test]
fn overwrite_splits_a_spanning_clip_and_keeps_both_pieces_linked_across_tracks() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 300, PlaceMode::Overwrite))
        .unwrap();
    timeline
        .apply(place(source, 100, 500, 50, PlaceMode::Overwrite))
        .unwrap();

    let expected = [(0, 100, 0), (100, 150, 500), (150, 300, 150)];
    assert_eq!(spans(&timeline, V1), expected);
    assert_eq!(spans(&timeline, A1), expected);
    let sequence = timeline.sequence();
    let link = |track: TrackRef, index: usize| sequence.track(track).unwrap().clips[index].link;
    for index in 0..3 {
        assert_eq!(link(V1, index), link(A1, index));
    }
    assert_ne!(link(V1, 0), link(V1, 2));
}

#[test]
fn insert_pushes_later_clips_on_every_unlocked_track() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 300, PlaceMode::Overwrite))
        .unwrap();
    let mut on_v2 = place(source, 400, 0, 100, PlaceMode::Overwrite);
    on_v2.video_track = Some(1);
    on_v2.audio_track = None;
    timeline.apply(on_v2).unwrap();

    timeline
        .apply(place(source, 100, 0, 50, PlaceMode::Insert))
        .unwrap();

    assert_eq!(
        spans(&timeline, V1),
        [(0, 100, 0), (100, 150, 0), (150, 350, 100)]
    );
    assert_eq!(spans(&timeline, V2), [(450, 550, 0)]);
}

#[test]
fn razor_on_a_clip_cuts_its_linked_partner_too() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 300, PlaceMode::Overwrite))
        .unwrap();
    let video = clip_on(&timeline, V1, 0);
    timeline
        .apply(Razor {
            at: Ticks(120),
            target: RazorTarget::Clips(vec![video]),
        })
        .unwrap();
    assert_eq!(spans(&timeline, A1), [(0, 120, 0), (120, 300, 120)]);
}

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

#[test]
fn move_overwrites_the_destination_and_creates_missing_tracks() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 100, PlaceMode::Overwrite))
        .unwrap();
    timeline
        .apply(place(source, 200, 0, 300, PlaceMode::Overwrite))
        .unwrap();
    let first = clip_on(&timeline, V1, 0);
    timeline
        .apply(MoveClips {
            clips: vec![first],
            delta: Ticks(250),
            video_tracks: 2,
            audio_tracks: 0,
        })
        .unwrap();
    assert_eq!(spans(&timeline, TrackRef::video(2)), [(250, 350, 0)]);
    assert_eq!(
        spans(&timeline, A1),
        [(200, 250, 0), (250, 350, 0), (350, 500, 150)]
    );
}

#[test]
fn ripple_delete_closes_the_gap_unless_another_track_blocks_it() {
    let (mut timeline, source) = timeline();
    for at in [0, 100, 200] {
        timeline
            .apply(place(source, at, 0, 100, PlaceMode::Overwrite))
            .unwrap();
    }
    let middle = clip_on(&timeline, V1, 1);
    // A clip on V2 spans the end of the gap, so the V2 clip after it can't move back.
    let mut blocker = place(source, 150, 0, 100, PlaceMode::Overwrite);
    blocker.video_track = Some(1);
    blocker.audio_track = None;
    let mut blocked = timeline.sequence().clone();

    timeline
        .apply(RippleDelete {
            clips: vec![middle],
        })
        .unwrap();
    assert_eq!(spans(&timeline, A1), [(0, 100, 0), (100, 200, 0)]);

    blocker.edit(&mut blocked).unwrap();
    blocker.at = Ticks(250);
    blocker.edit(&mut blocked).unwrap();
    let edit = RippleDelete {
        clips: vec![middle],
    };
    assert!(matches!(
        crate::preview(&blocked, &edit),
        Err(TimelineError::ClipOverlap { track: V2, .. })
    ));
}

#[test]
fn close_gap_pulls_later_clips_back() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 100, PlaceMode::Overwrite))
        .unwrap();
    timeline
        .apply(place(source, 300, 0, 100, PlaceMode::Overwrite))
        .unwrap();
    timeline
        .apply(CloseGap {
            track: V1,
            at: Ticks(150),
        })
        .unwrap();
    assert_eq!(spans(&timeline, A1), [(0, 100, 0), (100, 200, 0)]);
}

#[test]
fn edits_touching_a_locked_track_are_rejected() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 100, PlaceMode::Overwrite))
        .unwrap();
    timeline
        .apply(SetTrackSwitch {
            track: A1,
            switch: TrackSwitch::Lock,
            on: true,
        })
        .unwrap();
    let video = clip_on(&timeline, V1, 0);
    assert_eq!(
        timeline.apply(Lift { clips: vec![video] }),
        Err(TimelineError::TrackLocked(A1))
    );
}

#[test]
fn undo_and_redo_restore_each_state_exactly() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 300, PlaceMode::Overwrite))
        .unwrap();
    let before = timeline.sequence().clone();
    timeline
        .apply(place(source, 100, 0, 50, PlaceMode::Insert))
        .unwrap();
    let after = timeline.sequence().clone();

    assert!(timeline.undo());
    assert_eq!(timeline.sequence(), &before);
    assert!(timeline.redo().unwrap());
    assert_eq!(timeline.sequence(), &after);
    assert_eq!(timeline.undo_name(), Some("Insert"));
}

#[test]
fn going_to_a_step_undoes_or_redoes_everything_between() {
    let (mut timeline, source) = timeline();
    let opened = timeline.sequence().clone();
    timeline
        .apply(place(source, 0, 0, 300, PlaceMode::Overwrite))
        .unwrap();
    timeline
        .apply(place(source, 100, 0, 50, PlaceMode::Insert))
        .unwrap();
    let inserted = timeline.sequence().clone();

    timeline.go_to(1).unwrap();
    assert_eq!(timeline.sequence(), &opened);
    assert_eq!(
        timeline.undone().collect::<Vec<_>>(),
        ["Overwrite", "Insert"]
    );
    timeline.go_to(3).unwrap();
    assert_eq!(timeline.sequence(), &inserted);
    assert_eq!(
        timeline.done().collect::<Vec<_>>(),
        ["Import", "Overwrite", "Insert"]
    );
}

#[test]
fn failed_edit_leaves_sequence_and_history_untouched() {
    let (mut timeline, source) = timeline();
    let before = timeline.sequence().clone();
    let too_long = place(source, 0, 900, 200, PlaceMode::Overwrite);
    assert!(matches!(
        timeline.apply(too_long),
        Err(TimelineError::OutsideMedia(_))
    ));
    assert_eq!(timeline.sequence(), &before);
    assert_eq!(timeline.undo_name(), Some("Import"));
}

#[test]
fn clip_gain_reaches_linked_audio_but_not_video() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 300, PlaceMode::Overwrite))
        .unwrap();
    let gain = crate::Gain::from_db(-6.0);
    let video = clip_on(&timeline, V1, 0);
    timeline
        .apply(SetClipGain {
            clips: vec![video],
            gain,
        })
        .unwrap();
    let sequence = timeline.sequence();
    assert_eq!(sequence.track(A1).unwrap().clips[0].gain, gain);
    assert_eq!(
        sequence.track(V1).unwrap().clips[0].gain,
        crate::Gain::UNITY
    );
}

#[test]
fn proxy_replaces_only_the_decoded_path_and_undoes() {
    let (mut timeline, source) = timeline();
    let proxy = std::path::PathBuf::from("Proxies/a_Proxy.mp4");
    let attach = SetProxy {
        source,
        proxy: Some(proxy.clone()),
    };
    timeline.apply(attach.clone()).unwrap();
    assert_eq!(timeline.apply(attach), Err(TimelineError::NothingToEdit));

    let decoded = timeline.sequence().with_proxies();
    assert_eq!(decoded.media[0].path, proxy);
    assert_eq!(
        timeline.sequence().media[0].path,
        std::path::Path::new("a.mp4")
    );

    assert!(timeline.undo());
    assert_eq!(timeline.sequence().media[0].proxy, None);
}

mod bin;
mod caption;
mod enable;
mod generate;
mod hold;
mod link;
mod marker;
mod media;
mod nest;
mod paste;
mod range;
mod speed;
