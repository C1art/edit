use std::time::{Duration, Instant};

use timeline::{Ticks, Timebase};

/// How long to wait for a frame before seeking past it anyway, so slow media can't
/// stall the shuttle.
const FRAME_PATIENCE: Duration = Duration::from_millis(250);

const NANOSECONDS: Timebase = match Timebase::new(1, 1_000_000_000) {
    Some(timebase) => timebase,
    None => unreachable!(),
};

/// A playing speed in percent of normal, backwards when negative.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayRate(pub i32);

impl PlayRate {
    pub const STOPPED: Self = Self(0);
    pub const NORMAL: Self = Self(100);
}

/// Pictures-only playback off normal speed, backwards when negative. It runs on the
/// wall clock and seeks to each new position once the last one is shown.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Shuttle {
    rate: PlayRate,
    anchor: Ticks,
    started: Instant,
    last_seek: Instant,
}

impl Shuttle {
    pub(crate) fn new(rate: PlayRate, anchor: Ticks, now: Instant) -> Self {
        Self {
            rate,
            anchor,
            started: now,
            last_seek: now,
        }
    }

    pub(crate) fn rate(&self) -> PlayRate {
        self.rate
    }

    /// Where the shuttle has reached at `now`, within `0..=duration`.
    pub(crate) fn position(&self, now: Instant, timebase: Timebase, duration: Ticks) -> Ticks {
        let elapsed =
            i64::try_from(now.duration_since(self.started).as_nanos()).unwrap_or(i64::MAX);
        let played = Ticks(elapsed).rescale(NANOSECONDS, timebase);
        let moved = i128::from(played.0) * i128::from(self.rate.0) / 100;
        let moved = Ticks(i64::try_from(moved).unwrap_or(i64::MAX));
        (self.anchor + moved).clamp(Ticks::ZERO, duration)
    }

    /// Whether to seek again: the last seek's frame is on screen, or is taking too long.
    pub(crate) fn wants_seek(&self, shown: bool, now: Instant) -> bool {
        shown || now.duration_since(self.last_seek) >= FRAME_PATIENCE
    }

    pub(crate) fn sought(&mut self, now: Instant) {
        self.last_seek = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_at_its_rate_and_stops_at_the_ends() {
        let start = Instant::now();
        let second = Duration::from_secs(1);
        let reverse = Shuttle::new(PlayRate(-400), Ticks(900_000), start);
        let position = |shuttle: &Shuttle, after| {
            shuttle.position(start + after, Timebase::HZ_90K, Ticks(1_800_000))
        };
        assert_eq!(position(&reverse, second), Ticks(540_000));
        assert_eq!(position(&reverse, 3 * second), Ticks::ZERO);
        let forward = Shuttle::new(PlayRate(800), Ticks(900_000), start);
        assert_eq!(position(&forward, 2 * second), Ticks(1_800_000));
        let slow = Shuttle::new(PlayRate(10), Ticks(900_000), start);
        assert_eq!(position(&slow, second), Ticks(909_000));
    }
}
