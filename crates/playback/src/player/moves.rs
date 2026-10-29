use std::ops::Range;
use std::time::Instant;

use timeline::Ticks;

use super::{Player, Stop};
use crate::shuttle::Shuttle;

/// Moving the playhead other than by playing on: shuttling, ranges, loops, steps and
/// scrubs.
impl Player {
    /// Plays at `rate` times normal speed, backwards when negative. Off normal speed only
    /// pictures play, as when shuttling a deck.
    pub fn shuttle(&mut self, rate: i32) {
        match rate {
            0 => self.pause(),
            1 => self.play(),
            _ => {
                self.pause();
                self.shuttle = Some(Shuttle::new(rate, self.position(), Instant::now()));
            }
        }
    }

    /// Whether [`scrub`](Self::scrub) sounds the audio where it lands.
    pub fn set_scrub_audio(&mut self, on: bool) {
        self.scrub_audio = on;
    }

    /// Moves the playhead as dragging it does: while paused, a moment of the sound from
    /// there plays. Landing on the frame already there does nothing, so a pointer held
    /// still stays quiet.
    pub fn scrub(&mut self, position: Ticks) {
        let position = position.clamp(Ticks::ZERO, self.duration());
        let (rate, timebase) = (self.frame_rate(), self.sequence.timebase);
        if rate.frame_at(position, timebase) == rate.frame_at(self.position(), timebase) {
            return;
        }
        if self.is_playing() || !self.scrub_audio {
            self.seek(position);
            return;
        }
        self.stop = None;
        if let Some(writer) = self.writer.as_ref().filter(|writer| !writer.is_active()) {
            writer.claim();
        }
        self.go_to(position, true);
    }

    /// Plays `range` once, then stops, going back to `back_to` when given.
    pub fn play_range(&mut self, range: Range<Ticks>, back_to: Option<Ticks>) {
        if range.is_empty() {
            return;
        }
        self.pause();
        self.seek(range.start);
        self.play();
        self.stop = Some(Stop {
            at: range.end,
            back_to,
        });
    }

    pub fn is_looping(&self) -> bool {
        self.looping.is_some()
    }

    /// Loops `range` while playing, or plays on through when `None`.
    pub fn set_looping(&mut self, range: Option<Range<Ticks>>) {
        self.looping = range.filter(|range| !range.is_empty());
    }

    pub(super) fn keep_in_range(&mut self, position: Ticks) {
        if let Some(stop) = self.stop.filter(|stop| position >= stop.at) {
            self.pause();
            // The clock runs past the end by up to a frame; rest on the range's last one.
            let (rate, timebase) = (self.frame_rate(), self.sequence.timebase);
            let last = rate.frame_start(rate.frame_at(stop.at - Ticks(1), timebase), timebase);
            self.seek(stop.back_to.unwrap_or(last));
        } else if let Some(range) = self.looping.clone() {
            if position >= range.end {
                self.go_to(range.start, false);
            }
        }
    }

    pub(super) fn advance_shuttle(&mut self) {
        let Some(mut shuttle) = self.shuttle else {
            return;
        };
        let now = Instant::now();
        let shown = !self.awaiting_seek && self.picker.is_settled();
        if !shuttle.wants_seek(shown, now) {
            return;
        }
        let target = shuttle.position(now, self.sequence.timebase, self.duration());
        let at = self.clock.position(self.count());
        let frame = |time| self.frame_rate().frame_at(time, self.sequence.timebase);
        if frame(target) != frame(at) {
            shuttle.sought(now);
            self.go_to(target, false);
            self.shuttle = Some(shuttle);
        } else if target == Ticks::ZERO || target == self.duration() {
            self.shuttle = None;
        }
    }

    /// Moves by whole frames from the frame on screen, pausing first, and sounds the
    /// frame's audio as a scrub does.
    pub fn step(&mut self, frames: i64) {
        self.pause();
        let rate = self.frame_rate();
        let from = self
            .current_frame()
            .map_or(self.position(), |frame| frame.pts);
        let frame = rate
            .frame_at(from, self.sequence.timebase)
            .saturating_add(frames);
        self.scrub(rate.frame_start(frame, self.sequence.timebase));
    }
}
