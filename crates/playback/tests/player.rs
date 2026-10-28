// Helpers outside `#[test]` fns are not covered by `allow-unwrap-in-tests`.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use media::MediaReader;
use playback::{LayerPicture, Player};
use timeline::{
    FrameRate, ImportMedia, PlaceClip, PlaceMode, Sequence, Ticks, Timebase, Timeline, VideoFormat,
};

const TB: Timebase = Timebase::HZ_90K;
const FRAME: i64 = 90_000 / 25;

/// The fixture placed twice back to back, the second time from its one-second mark.
fn sequence() -> Arc<Sequence> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/testsrc_2s_25fps_44k.mp4");
    let info = MediaReader::open(&path, TB).unwrap().info().clone();
    let format = VideoFormat {
        width: 320,
        height: 240,
        frame_rate: FrameRate::FPS_25,
        ..VideoFormat::default()
    };
    let mut timeline = Timeline::new(Sequence::with_default_tracks(TB, format));
    timeline
        .apply(ImportMedia {
            name: "testsrc".into(),
            path,
            duration: info.duration.unwrap(),
            video: Some(format),
            has_audio: true,
            still: false,
        })
        .unwrap();
    let source = timeline.sequence().media[0].id;
    for (at, source_in) in [(0, 0), (50 * FRAME, 25 * FRAME)] {
        timeline
            .apply(PlaceClip {
                source,
                source_in: Ticks(source_in),
                duration: Ticks(25 * FRAME),
                at: Ticks(at),
                video_track: Some(0),
                audio_track: Some(0),
                mode: PlaceMode::Overwrite,
                fill: None,
            })
            .unwrap();
    }
    Arc::new(timeline.sequence().clone())
}

fn pump_until(player: &mut Player, mut done: impl FnMut(&Player) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !done(player) {
        assert!(Instant::now() < deadline, "timed out");
        player.update();
        assert!(player.take_warnings().is_empty());
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn shown(player: &Player) -> Option<(Ticks, usize)> {
    player
        .current_frame()
        .map(|frame| (frame.pts, frame.layers.len()))
}

#[test]
#[ignore = "decodes a real file with FFmpeg in real time"]
fn plays_across_a_gap_to_the_end_on_the_wall_clock() {
    let mut player = Player::new(sequence(), None).unwrap();
    pump_until(&mut player, |player| player.current_frame().is_some());
    player.play();
    let started = Instant::now();
    pump_until(&mut player, |player| !player.is_playing());
    let elapsed = started.elapsed();
    assert!(elapsed > Duration::from_millis(2900), "{elapsed:?}");
    assert_eq!(shown(&player), Some((Ticks(74 * FRAME), 1)));
    assert_eq!(player.position(), Ticks(75 * FRAME));
}

#[test]
#[ignore = "decodes a real file with FFmpeg"]
fn seeks_show_the_clip_frame_or_black_in_gaps() {
    let mut player = Player::new(sequence(), None).unwrap();
    player.seek(Ticks(60 * FRAME + 10));
    pump_until(&mut player, |player| {
        shown(player) == Some((Ticks(60 * FRAME), 1))
    });
    let frame = player.current_frame().unwrap();
    let LayerPicture::Frame(picture) = &frame.layers[0].picture else {
        panic!("a file clip shows a decoded frame");
    };
    assert_eq!(picture.pts, Ticks(35 * FRAME));

    player.step(-25);
    pump_until(&mut player, |player| {
        shown(player) == Some((Ticks(35 * FRAME), 0))
    });
}
