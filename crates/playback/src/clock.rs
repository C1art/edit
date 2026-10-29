use timeline::{Ticks, Timebase};

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
}

impl PlaybackClock {
    pub(crate) fn new(timebase: Timebase, count_timebase: Timebase) -> Self {
        Self {
            timebase,
            count_timebase,
            anchor: Ticks::ZERO,
            anchor_count: 0,
            running: false,
        }
    }

    pub(crate) fn position(&self, count: u64) -> Ticks {
        if !self.running {
            return self.anchor;
        }
        let elapsed = i64::try_from(count.saturating_sub(self.anchor_count)).unwrap_or(i64::MAX);
        self.anchor + Ticks(elapsed).rescale(self.count_timebase, self.timebase)
    }

    pub(crate) fn start(&mut self, count: u64) {
        self.anchor_count = count;
        self.running = true;
    }

    pub(crate) fn stop(&mut self, count: u64) {
        self.anchor = self.position(count);
        self.running = false;
    }

    /// Stops the clock at `position`; it holds there until the next `start`.
    pub(crate) fn jump(&mut self, position: Ticks) {
        self.anchor = position;
        self.running = false;
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
        clock.jump(Ticks(90_000));
        clock.start(1_000_000);
        assert_eq!(clock.position(1_000_000 + 24_000), Ticks(135_000));
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
        clock.jump(Ticks(500));
        clock.start(100);
        assert_eq!(clock.position(50), Ticks(500));
    }
}
