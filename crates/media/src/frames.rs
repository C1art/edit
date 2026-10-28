use timeline::{Ticks, Timebase};

/// A decoded picture as tightly packed RGBA8, still gamma-encoded in the source transfer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoFrame {
    /// In the reader's output timebase, relative to the start of the media.
    pub pts: Ticks,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

/// Interleaved `f32` samples at the source sample rate.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioBuffer {
    /// In the reader's output timebase, relative to the start of the media.
    pub pts: Ticks,
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

impl AudioBuffer {
    pub fn frames(&self) -> usize {
        self.samples.len() / usize::from(self.channels.max(1))
    }

    pub fn end(&self, timebase: Timebase) -> Ticks {
        self.pts + self.frames_to_ticks(self.frames(), timebase)
    }

    fn sample_timebase(&self) -> Option<Timebase> {
        Timebase::new(1, self.sample_rate)
    }

    fn frames_to_ticks(&self, frames: usize, timebase: Timebase) -> Ticks {
        let frames = Ticks(i64::try_from(frames).unwrap_or(i64::MAX));
        self.sample_timebase()
            .map_or(Ticks::ZERO, |rate| frames.rescale(rate, timebase))
    }

    /// Drops the samples that play before `position`.
    pub(crate) fn trim_start_to(&mut self, position: Ticks, timebase: Timebase) {
        let Some(rate) = self.sample_timebase() else {
            return;
        };
        let skip = (position - self.pts).rescale(timebase, rate).0;
        let skip = usize::try_from(skip).unwrap_or(0).min(self.frames());
        self.samples.drain(..skip * usize::from(self.channels));
        self.pts = position;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DecodedFrame {
    Video(VideoFrame),
    Audio(AudioBuffer),
}
