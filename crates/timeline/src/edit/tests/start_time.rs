use super::*;

/// 90 kHz ticks per frame at 25 fps.
const FRAME: i64 = 3_600;

#[test]
fn start_time_lands_on_a_frame_and_undoes() {
    let (mut timeline, _) = timeline();
    timeline
        .apply(SetStartTime {
            start: Ticks(90_000 * 3_600 + FRAME / 2),
        })
        .unwrap();
    assert_eq!(timeline.sequence().start_time, Ticks(90_000 * 3_600));
    assert_eq!(
        timeline.sequence().timecode_at(Ticks(FRAME)).to_string(),
        "01:00:00:01"
    );
    assert_eq!(
        timeline.apply(SetStartTime {
            start: Ticks(90_000 * 3_600)
        }),
        Err(TimelineError::NothingToEdit)
    );
    timeline.undo();
    assert_eq!(timeline.sequence().start_time, Ticks::ZERO);
}
