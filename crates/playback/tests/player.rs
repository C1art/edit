// Helpers outside `#[test]` fns are not covered by `allow-unwrap-in-tests`.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use playback::Player;
use timeline::{Ticks, Timebase};

const TB: Timebase = Timebase::HZ_90K;
const FRAME: i64 = 90_000 / 25;

fn open() -> Player {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/testsrc_2s_25fps_44k.mp4");
    let mut player = Player::open(path, TB, None).unwrap();
    pump_until(&mut player, |player| player.info().is_some());
    player
}

fn pump_until(player: &mut Player, mut done: impl FnMut(&Player) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !done(player) {
        assert!(Instant::now() < deadline, "timed out");
        assert!(player.error().is_none(), "{:?}", player.error());
        player.update();
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn shown_pts(player: &Player) -> Option<Ticks> {
    player.current_frame().map(|frame| frame.pts)
}

#[test]
#[ignore = "decodes a real file with FFmpeg in real time"]
fn plays_to_the_end_on_the_wall_clock_and_stops_on_the_last_frame() {
    let mut player = open();
    player.play();
    let started = Instant::now();
    let mut shown = 0;
    pump_until(&mut player, |player| {
        shown += 1;
        !player.is_playing()
    });
    let elapsed = started.elapsed();
    assert!(elapsed > Duration::from_millis(1900), "{elapsed:?}");
    assert_eq!(shown_pts(&player), Some(Ticks(49 * FRAME)));
    assert_eq!(player.position(), player.duration().unwrap());
}

#[test]
#[ignore = "decodes a real file with FFmpeg"]
fn paused_seek_and_step_show_the_exact_frame() {
    let mut player = open();
    player.seek(Ticks(30 * FRAME + 10));
    pump_until(&mut player, |player| {
        shown_pts(player) == Some(Ticks(30 * FRAME))
    });
    player.step(-3);
    pump_until(&mut player, |player| {
        shown_pts(player) == Some(Ticks(27 * FRAME))
    });
    assert!(!player.is_playing());
}
