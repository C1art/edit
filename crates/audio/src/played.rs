use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Marks `at` while the callback is moving the count, so a reader never pairs a new
/// count with the old time.
const UPDATING: u64 = u64::MAX;

/// Frames the device has played, moving smoothly between callbacks. The device takes
/// audio a buffer at a time, every 10 ms or so; a display sampling that count at 60 Hz
/// sees it step unevenly and skips or repeats frames to follow it.
#[derive(Debug)]
pub(crate) struct Played {
    epoch: Instant,
    rate: u64,
    frames: AtomicU64,
    /// Nanoseconds after `epoch` of the last callback.
    at: AtomicU64,
    /// Frames the last callback played: the count runs on from it at the sample rate
    /// for no further, as the next callback plays about as many.
    chunk: AtomicU64,
    /// The most any reader was told, so the count never goes back when a callback
    /// plays less than the one before, as when the queue runs short.
    told: AtomicU64,
}

impl Played {
    pub(crate) fn new(rate: u32) -> Self {
        Self {
            epoch: Instant::now(),
            rate: u64::from(rate),
            frames: AtomicU64::new(0),
            at: AtomicU64::new(0),
            chunk: AtomicU64::new(0),
            told: AtomicU64::new(0),
        }
    }

    /// For the audio callback: lock-free and without allocating.
    pub(crate) fn advance(&self, frames: u64) {
        self.advance_at(frames, self.now());
    }

    pub(crate) fn frames(&self) -> u64 {
        self.frames_at(self.now())
    }

    fn now(&self) -> u64 {
        u64::try_from(self.epoch.elapsed().as_nanos()).unwrap_or(UPDATING - 1)
    }

    fn advance_at(&self, frames: u64, now: u64) {
        self.at.store(UPDATING, Ordering::SeqCst);
        self.frames.fetch_add(frames, Ordering::SeqCst);
        self.chunk.store(frames, Ordering::SeqCst);
        self.at.store(now.min(UPDATING - 1), Ordering::SeqCst);
    }

    fn frames_at(&self, now: u64) -> u64 {
        let (frames, at, chunk) = loop {
            let at = self.at.load(Ordering::SeqCst);
            if at == UPDATING {
                std::hint::spin_loop();
                continue;
            }
            let frames = self.frames.load(Ordering::SeqCst);
            let chunk = self.chunk.load(Ordering::SeqCst);
            if self.at.load(Ordering::SeqCst) == at {
                break (frames, at, chunk);
            }
        };
        let elapsed = u128::from(now.saturating_sub(at));
        let since = u64::try_from(elapsed * u128::from(self.rate) / 1_000_000_000)
            .unwrap_or(u64::MAX)
            .min(chunk);
        let frames = frames + since;
        self.told.fetch_max(frames, Ordering::SeqCst).max(frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: u64 = 1_000_000;

    #[test]
    fn runs_on_between_callbacks_up_to_one_buffer() {
        let played = Played::new(48_000);
        played.advance_at(480, 10 * MS);
        assert_eq!(played.frames_at(10 * MS), 480);
        assert_eq!(played.frames_at(15 * MS), 720);
        assert_eq!(played.frames_at(40 * MS), 960);
    }

    #[test]
    fn never_goes_back_when_a_callback_plays_less() {
        let played = Played::new(48_000);
        played.advance_at(480, 10 * MS);
        assert_eq!(played.frames_at(19 * MS), 912);
        played.advance_at(100, 20 * MS);
        assert_eq!(played.frames_at(20 * MS), 912);
        assert_eq!(played.frames_at(21 * MS), 912);
    }

    #[test]
    fn stands_still_while_nothing_plays() {
        let played = Played::new(48_000);
        played.advance_at(480, 10 * MS);
        played.advance_at(0, 20 * MS);
        assert_eq!(played.frames_at(50 * MS), 480);
    }
}
