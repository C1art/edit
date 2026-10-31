use super::*;
use crate::{pauses, PauseRules, RemovePauses};

#[test]
fn pauses_are_whole_frames_of_long_enough_quiet_less_padding_beside_sound() {
    // 100 windows a second at 25 fps: a frame is four windows.
    let mut levels = vec![-60.0; 100];
    for window in (0..10).chain(50..60).chain(62..70) {
        levels[window] = -20.0;
    }
    let rules = PauseRules {
        threshold_db: -40.0,
        shortest: Ticks(9_000),
        padding: Ticks(1_800),
    };
    let found = pauses(
        &levels,
        (Ticks::ZERO, Ticks(900)),
        rules,
        vec![Ticks::ZERO..Ticks(90_000)],
        (FrameRate::FPS_25, Timebase::HZ_90K),
    );
    assert_eq!(
        found,
        [Ticks(10_800)..Ticks(43_200), Ticks(64_800)..Ticks(90_000)]
    );
}

#[test]
fn removing_pauses_closes_them_up_or_leaves_gaps() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 1000, PlaceMode::Overwrite))
        .unwrap();
    let remove = |ripple| RemovePauses {
        pauses: vec![Ticks(500)..Ticks(600), Ticks(100)..Ticks(200)],
        tracks: vec![V1, A1],
        ripple,
    };
    timeline.apply(remove(false)).unwrap();
    let gapped = [(0, 100, 0), (200, 500, 200), (600, 1000, 600)];
    assert_eq!(spans(&timeline, A1), gapped);
    timeline.undo();
    timeline.apply(remove(true)).unwrap();
    let closed = [(0, 100, 0), (100, 400, 200), (400, 800, 600)];
    assert_eq!(spans(&timeline, V1), closed);
    assert_eq!(spans(&timeline, A1), closed);
}
