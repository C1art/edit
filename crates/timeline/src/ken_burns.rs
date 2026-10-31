//! The Ken Burns move: what part of a clip's picture its frame shows at the clip's first
//! frame and at its last, set by keyframing scale and position there.

use crate::{Clip, FrameRate, Motion, Param, Ticks, Timebase};

/// The part of a picture a frame shows: its center, from the picture's center in fitted
/// picture pixels, +y down, and how wide it is in them; it is as tall for its width as
/// the frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameRegion {
    pub center: [f32; 2],
    pub width: f32,
}

/// Which end of a Ken Burns move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MoveEnd {
    Start,
    End,
}

/// The source times of a clip's first and last frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KenBurns {
    pub first: Ticks,
    pub last: Ticks,
}

impl KenBurns {
    /// `None` for a clip of a single frame, which has nowhere to move.
    pub fn of(clip: &Clip, rate: FrameRate, timebase: Timebase) -> Option<Self> {
        let frame_at = |position: Ticks| rate.frame_at(position, timebase);
        let (first, last) = (frame_at(clip.start), frame_at(clip.end() - Ticks(1)));
        if last <= first {
            return None;
        }
        let [first, last] = [first, last]
            .map(|frame| clip.source_time(rate.frame_start(frame, timebase).max(clip.start)));
        Some(Self { first, last })
    }

    fn time(&self, end: MoveEnd) -> Ticks {
        match end {
            MoveEnd::Start => self.first,
            MoveEnd::End => self.last,
        }
    }

    /// The framings at the start and at the end, in a frame `frame_width` pixels wide;
    /// `None` if the picture is turned or unevenly scaled at either.
    pub fn framings(&self, motion: &Motion, frame_width: f32) -> Option<[FrameRegion; 2]> {
        Some([
            motion.framing_at(self.first, frame_width)?,
            motion.framing_at(self.last, frame_width)?,
        ])
    }

    /// Keys scale and position at both ends, so the other end holds where it was, and
    /// sets `end` to show `framing`.
    pub fn set(&self, motion: &mut Motion, end: MoveEnd, framing: FrameRegion, frame_width: f32) {
        let uniform = motion.uniform_scale;
        let [x, y] = &mut motion.position;
        let scales = [&mut motion.scale, &mut motion.scale_width];
        let keyed = scales.into_iter().take(if uniform { 1 } else { 2 });
        for param in keyed.chain([x, y]) {
            key_both_ends(param, [self.first, self.last]);
        }
        motion.frame_at(self.time(end), framing, frame_width);
    }

    /// Exchanges what the start and end show; false if either can't be said as a framing.
    pub fn swap(&self, motion: &mut Motion, frame_width: f32) -> bool {
        let Some([start, end]) = self.framings(motion, frame_width) else {
            return false;
        };
        self.set(motion, MoveEnd::Start, end, frame_width);
        self.set(motion, MoveEnd::End, start, frame_width);
        true
    }
}

fn key_both_ends(param: &mut Param, ends: [Ticks; 2]) {
    for time in ends {
        let value = param.value_at(time);
        if !param.is_animated() {
            param.toggle_animation(time);
        } else if !param.has_key_at(time) {
            param.set_at(time, value);
        }
    }
}

impl Motion {
    /// What a frame `frame_width` pixels wide shows of the picture at `time`; `None`
    /// while the picture is turned or unevenly scaled, which a framing can't describe.
    pub fn framing_at(&self, time: Ticks, frame_width: f32) -> Option<FrameRegion> {
        let [width, height] = self.scale_at(time);
        if width != height || height <= 0.0 || self.rotation.value_at(time) != 0.0 {
            return None;
        }
        let scale = height / 100.0;
        let center = [0, 1].map(|axis| {
            self.anchor[axis].value_at(time) - self.position[axis].value_at(time) / scale
        });
        Some(FrameRegion {
            center,
            width: frame_width / scale,
        })
    }

    /// Scales and places the picture at `time` so the frame shows `framing`.
    fn frame_at(&mut self, time: Ticks, framing: FrameRegion, frame_width: f32) {
        let percent = 100.0 * frame_width / framing.width.max(f32::EPSILON);
        self.scale.set_at(time, percent);
        self.scale_width.set_at(time, percent);
        // Placed by the scale as kept, so rounding it doesn't shift the framing.
        let scale = self.scale.value_at(time) / 100.0;
        for axis in 0..2 {
            let anchor = self.anchor[axis].value_at(time);
            let offset = scale * (anchor - framing.center[axis]);
            self.position[axis].set_at(time, offset);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIDTH: f32 = 1920.0;

    fn near(a: FrameRegion, b: FrameRegion) -> bool {
        let close = |a: f32, b: f32| (a - b).abs() < 0.05;
        close(a.width, b.width)
            && close(a.center[0], b.center[0])
            && close(a.center[1], b.center[1])
    }

    #[test]
    fn setting_one_end_keeps_the_other_and_reads_back() {
        let span = KenBurns {
            first: Ticks(0),
            last: Ticks(1_000),
        };
        let mut motion = Motion::default();
        motion.anchor[0] = Param::from_value(40.0);
        let start = motion.framing_at(Ticks(0), WIDTH).unwrap();
        let wanted = FrameRegion {
            center: [300.0, -120.0],
            width: 960.0,
        };
        span.set(&mut motion, MoveEnd::End, wanted, WIDTH);
        let [read_start, read_end] = span.framings(&motion, WIDTH).unwrap();
        assert!(near(read_start, start), "{read_start:?}");
        assert!(near(read_end, wanted), "{read_end:?}");
        assert_eq!(motion.scale.value_at(Ticks(1_000)), 200.0);

        assert!(span.swap(&mut motion, WIDTH));
        let [swapped_start, swapped_end] = span.framings(&motion, WIDTH).unwrap();
        assert!(near(swapped_start, wanted) && near(swapped_end, start));
    }
}
