use std::sync::atomic::{AtomicI64, Ordering};

use timeline::{Ticks, Timebase};

const NOT_PLAYING: i64 = i64::MIN;

/// Where playing has reached, or nothing while it isn't playing, shared with the video
/// worker so that one falling behind can skip the frames it would only make late.
#[derive(Debug)]
pub(crate) struct SharedPosition(AtomicI64);

impl SharedPosition {
    pub(crate) fn new() -> Self {
        Self(AtomicI64::new(NOT_PLAYING))
    }

    pub(crate) fn set(&self, position: Option<Ticks>) {
        let value = position.map_or(NOT_PLAYING, |position| position.0);
        self.0.store(value, Ordering::Relaxed);
    }

    pub(crate) fn get(&self) -> Option<Ticks> {
        let value = self.0.load(Ordering::Relaxed);
        (value != NOT_PLAYING).then_some(Ticks(value))
    }
}

/// Maps a monotonically increasing counter onto media time. The counter is the number of
/// audio frames the device has played (the master clock) or, without audio, wall-clock
/// nanoseconds.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PlaybackClock {
    timebase: Timebase,
    count_timebase: Timebase,
    anchor: Ticks,
    anchor_count: u64,
    running: bool,
    /// Media time passed per counted time, as a percentage.
    percent: i64,
}

impl PlaybackClock {
    pub(crate) fn new(timebase: Timebase, count_timebase: Timebase) -> Self {
        Self {
            timebase,
            count_timebase,
            anchor: Ticks::ZERO,
            anchor_count: 0,
            running: false,
            percent: 100,
        }
    }

    pub(crate) fn position(&self, count: u64) -> Ticks {
        if !self.running {
            return self.anchor;
        }
        let elapsed = i64::try_from(count.saturating_sub(self.anchor_count)).unwrap_or(i64::MAX);
        let elapsed = Ticks(elapsed).rescale(self.count_timebase, self.timebase);
        self.anchor + Ticks(elapsed.0.saturating_mul(self.percent) / 100)
    }

    pub(crate) fn start(&mut self, count: u64) {
        self.anchor_count = count;
        self.running = true;
    }

    pub(crate) fn stop(&mut self, count: u64) {
        self.anchor = self.position(count);
        self.running = false;
    }

    /// Stops the clock at `position`; it holds there until the next `start`, then runs
    /// at `percent` of the counter's pace.
    pub(crate) fn jump(&mut self, position: Ticks, percent: i64) {
        self.anchor = position;
        self.running = false;
        self.percent = percent;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn audio_clock() -> PlaybackClock {
        PlaybackClock::new(Timebase::HZ_90K, Timebase::new(1, 48_000).unwrap())
    }

    #[test]
    fn advances_from_the_count_at_start() {
        let mut clock = audio_clock();
        clock.jump(Ticks(90_000), 100);
        clock.start(1_000_000);
        assert_eq!(clock.position(1_000_000 + 24_000), Ticks(135_000));
    }

    #[test]
    fn runs_at_its_speed() {
        let mut clock = audio_clock();
        clock.jump(Ticks(90_000), 150);
        clock.start(0);
        assert_eq!(clock.position(48_000), Ticks(225_000));
    }

    #[test]
    fn holds_position_across_stop_and_restart() {
        let mut clock = audio_clock();
        clock.start(0);
        clock.stop(48_000);
        assert_eq!(clock.position(96_000), Ticks(90_000));
        clock.start(200_000);
        assert_eq!(clock.position(248_000), Ticks(180_000));
    }

    #[test]
    fn a_counter_behind_the_anchor_does_not_rewind() {
        let mut clock = audio_clock();
        clock.jump(Ticks(500), 100);
        clock.start(100);
        assert_eq!(clock.position(50), Ticks(500));
    }
}
