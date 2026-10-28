use std::ops::Range;

use timeline::{MediaItem, Sequence, Speed};

use crate::stretch::Wsola;
use crate::track_audio::TrackAudio;
use crate::PlaybackError;

const CHANNELS: usize = 2;
/// Source frames read at once when playing backwards, a second at 48 kHz.
const BACKWARD_BLOCK: i64 = 48_000;

/// Stereo frames of a media item's audio at the output rate.
pub(crate) trait FrameSource {
    /// Adds `out.len() / 2` frames starting at source frame `first` into `out`. A
    /// nested sequence finds the media its clips use in `pool`.
    fn read(
        &mut self,
        pool: &Sequence,
        media: &MediaItem,
        first: i64,
        out: &mut [f32],
    ) -> Result<(), PlaybackError>;
    /// Forgets the read position, so the next read seeks.
    fn reset(&mut self);
}

impl FrameSource for TrackAudio {
    fn read(
        &mut self,
        _: &Sequence,
        media: &MediaItem,
        first: i64,
        out: &mut [f32],
    ) -> Result<(), PlaybackError> {
        self.mix_into(media, first, out)
    }

    fn reset(&mut self) {
        TrackAudio::reset(self);
    }
}

/// Where a clip's audio comes from, in frames at the output rate.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ClipRead {
    /// The first source frame of the clip's range, which a reversed clip plays last.
    pub source_in: i64,
    /// Output frames in the clip; a reversed clip counts back from its end.
    pub length: i64,
    pub speed: Speed,
    pub reverse: bool,
    pub maintain_pitch: bool,
}

impl ClipRead {
    /// The source position heard `frame` output frames into the clip, as a whole frame
    /// and the fraction past it.
    pub(crate) fn position(&self, frame: i64) -> (i64, f32) {
        let (num, den) = (i128::from(self.speed.num()), i128::from(self.speed.den()));
        let (travelled, before) = if self.reverse {
            (i128::from(self.length - frame) * num, 1)
        } else {
            (i128::from(frame) * num, 0)
        };
        let whole = i128::from(self.source_in) + travelled.div_euclid(den) - before;
        let fraction = travelled.rem_euclid(den) as f64 / den as f64;
        (i64::try_from(whole).unwrap_or(i64::MAX), fraction as f32)
    }

    pub(crate) fn direction(&self) -> i64 {
        if self.reverse {
            -1
        } else {
            1
        }
    }
}

/// One clip's audio at its speed and direction. Off normal speed the source is
/// interpolated, so its pitch follows the speed as on tape, unless the clip maintains
/// pitch.
pub(crate) struct ClipAudio<S> {
    source: S,
    /// Source frames from `window_start` on, kept so interpolation reaches across chunks.
    window: Vec<f32>,
    window_start: i64,
    stretch: Wsola,
}

impl<S: FrameSource> ClipAudio<S> {
    pub(crate) fn new(source: S) -> Self {
        Self {
            source,
            window: Vec::new(),
            window_start: 0,
            stretch: Wsola::default(),
        }
    }

    pub(crate) fn reset(&mut self) {
        self.source.reset();
        self.window.clear();
        self.stretch.reset();
    }

    /// Adds `out.len() / 2` frames into `out`, starting `offset` output frames into the
    /// clip. Frames before the media starts stay silent.
    pub(crate) fn mix_into(
        &mut self,
        pool: &Sequence,
        media: &MediaItem,
        read: &ClipRead,
        offset: i64,
        out: &mut [f32],
    ) -> Result<(), PlaybackError> {
        let frames = (out.len() / CHANNELS) as i64;
        if frames == 0 {
            return Ok(());
        }
        if read.speed.is_normal() && !read.reverse {
            let first = read.position(offset).0;
            let skip = (-first).clamp(0, frames);
            let out = &mut out[usize::try_from(skip).unwrap_or(0) * CHANNELS..];
            return self.source.read(pool, media, first + skip, out);
        }
        if read.maintain_pitch && !read.speed.is_normal() {
            let needed = Wsola::needed(read, offset..offset + frames);
            self.fill(pool, media, needed, read.reverse)?;
            let (window, start) = (&self.window, self.window_start);
            self.stretch.render(read, offset, out, |frame, channel| {
                window_sample(window, start, frame, channel)
            });
            return Ok(());
        }
        let ends = [offset, offset + frames - 1].map(|frame| read.position(frame).0);
        let (low, high) = (ends[0].min(ends[1]), ends[0].max(ends[1]));
        if high < 0 {
            return Ok(());
        }
        self.fill(pool, media, (low - 1).max(0)..high + 3, read.reverse)?;
        for (frame, pair) in (offset..).zip(out.as_chunks_mut::<CHANNELS>().0) {
            let (index, t) = read.position(frame);
            if index < 0 {
                continue;
            }
            for (channel, sample) in pair.iter_mut().enumerate() {
                let at =
                    |frame: i64| window_sample(&self.window, self.window_start, frame, channel);
                *sample += cubic(at(index - 1), at(index), at(index + 1), at(index + 2), t);
            }
        }
        Ok(())
    }

    /// Makes the window hold `needed`. Forwards, it reads on from its end where it can so
    /// the source keeps decoding instead of seeking; backwards, it reads a block ending
    /// at `needed` so the requests that follow are already there.
    fn fill(
        &mut self,
        pool: &Sequence,
        media: &MediaItem,
        needed: Range<i64>,
        backward: bool,
    ) -> Result<(), PlaybackError> {
        let end = |window: &Self| window.window_start + (window.window.len() / CHANNELS) as i64;
        if backward {
            let inside = !self.window.is_empty()
                && needed.start >= self.window_start
                && needed.end <= end(self);
            if !inside {
                let start = (needed.start - BACKWARD_BLOCK).max(0);
                let frames = usize::try_from(needed.end - start).unwrap_or(0);
                self.window.clear();
                self.window.resize(frames * CHANNELS, 0.0);
                self.window_start = start;
                self.source.read(pool, media, start, &mut self.window)?;
            }
            return Ok(());
        }
        if needed.start < self.window_start || needed.start > end(self) || self.window.is_empty() {
            self.window.clear();
            self.window_start = needed.start;
        }
        let have = end(self);
        if needed.end > have {
            let old = self.window.len();
            let more = usize::try_from(needed.end - have).unwrap_or(0) * CHANNELS;
            self.window.resize(old + more, 0.0);
            self.source
                .read(pool, media, have, &mut self.window[old..])?;
        }
        let stale = usize::try_from(needed.start - self.window_start).unwrap_or(0) * CHANNELS;
        self.window.drain(..stale.min(self.window.len()));
        self.window_start = needed.start;
        Ok(())
    }
}

/// Clamped to the window, which repeats the edge frames where the media ends.
fn window_sample(window: &[f32], start: i64, frame: i64, channel: usize) -> f32 {
    let frames = (window.len() / CHANNELS) as i64;
    let index = (frame - start).clamp(0, (frames - 1).max(0));
    let index = usize::try_from(index).unwrap_or(0) * CHANNELS + channel;
    window.get(index).copied().unwrap_or(0.0)
}

/// Catmull-Rom interpolation at `t` between `b` and `c`; exact on straight lines.
fn cubic(a: f32, b: f32, c: f32, d: f32, t: f32) -> f32 {
    let slope_b = (c - a) / 2.0;
    let slope_c = (d - b) / 2.0;
    let t2 = t * t;
    let t3 = t2 * t;
    (2.0 * t3 - 3.0 * t2 + 1.0) * b
        + (t3 - 2.0 * t2 + t) * slope_b
        + (-2.0 * t3 + 3.0 * t2) * c
        + (t3 - t2) * slope_c
}

#[cfg(test)]
mod tests {
    use super::*;
    use timeline::{SourceId, Ticks, Timebase};

    /// Each frame's value is its frame number, and every read's start is recorded.
    #[derive(Default)]
    struct Ramp {
        reads: Vec<i64>,
    }

    impl FrameSource for Ramp {
        fn read(
            &mut self,
            _: &Sequence,
            _: &MediaItem,
            first: i64,
            out: &mut [f32],
        ) -> Result<(), PlaybackError> {
            self.reads.push(first);
            for (frame, pair) in (first..).zip(out.as_chunks_mut::<CHANNELS>().0) {
                pair.iter_mut().for_each(|sample| *sample += frame as f32);
            }
            Ok(())
        }

        fn reset(&mut self) {}
    }

    fn media() -> MediaItem {
        MediaItem {
            id: SourceId(1),
            name: "ramp".into(),
            path: "ramp.wav".into(),
            duration: Ticks(1_000_000),
            video: None,
            has_audio: true,
            proxy: None,
            bin: None,
            generator: None,
            nest: None,
            subclip: None,
        }
    }

    fn pool() -> Sequence {
        Sequence::new(Timebase::HZ_90K, Default::default())
    }

    fn forward(source_in: i64, speed: Speed) -> ClipRead {
        ClipRead {
            source_in,
            length: 1_000,
            speed,
            reverse: false,
            maintain_pitch: false,
        }
    }

    fn left(out: &[f32]) -> Vec<f32> {
        out.iter().step_by(CHANNELS).copied().collect()
    }

    #[test]
    fn chunks_at_an_odd_speed_read_the_source_straight_through() {
        let speed = Speed::new(3, 2).unwrap();
        let mut clip = ClipAudio::new(Ramp::default());
        let mut chunks = Vec::new();
        for chunk in 0..3 {
            let mut out = vec![0.0; 4 * CHANNELS];
            clip.mix_into(&pool(), &media(), &forward(100, speed), chunk * 4, &mut out)
                .unwrap();
            chunks.extend(left(&out));
        }
        let expected: Vec<f32> = (0..12).map(|k| 100.0 + k as f32 * 1.5).collect();
        assert_eq!(chunks, expected);
        let reads = &clip.source.reads;
        assert_eq!(reads[0], 99);
        assert!(reads.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(reads.len(), 3);
    }

    #[test]
    fn frames_before_the_media_starts_are_silent() {
        let mut clip = ClipAudio::new(Ramp::default());
        let mut out = vec![0.0; 6 * CHANNELS];
        let read = forward(0, Speed::new(1, 2).unwrap());
        clip.mix_into(&pool(), &media(), &read, -4, &mut out)
            .unwrap();
        let left = left(&out);
        assert_eq!(left[..5], [0.0; 5]);
        assert!(left[5] > 0.0);
    }

    #[test]
    fn reversed_audio_plays_the_range_backwards_reading_ahead_in_blocks() {
        let read = ClipRead {
            reverse: true,
            ..forward(100, Speed::NORMAL)
        };
        let mut clip = ClipAudio::new(Ramp::default());
        let mut heard = Vec::new();
        for chunk in 0..3 {
            let mut out = vec![0.0; 4 * CHANNELS];
            clip.mix_into(&pool(), &media(), &read, chunk * 4, &mut out)
                .unwrap();
            heard.extend(left(&out));
        }
        let expected: Vec<f32> = (0..12).map(|k| 1_099.0 - k as f32).collect();
        assert_eq!(heard, expected);
        assert_eq!(clip.source.reads, [0]);
    }
}
