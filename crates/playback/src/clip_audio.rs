use std::ops::Range;

use timeline::{MediaItem, Speed};

use crate::track_audio::TrackAudio;
use crate::PlaybackError;

const CHANNELS: usize = 2;

/// Stereo frames of a file's audio at the output rate.
pub(crate) trait FrameSource {
    /// Adds `out.len() / 2` frames starting at source frame `first` into `out`.
    fn read(&mut self, media: &MediaItem, first: i64, out: &mut [f32])
        -> Result<(), PlaybackError>;
    /// Forgets the read position, so the next read seeks.
    fn reset(&mut self);
}

impl FrameSource for TrackAudio {
    fn read(
        &mut self,
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

/// One clip's audio at its speed. Off normal speed the source is read straight through
/// and interpolated, so its pitch follows the speed as on tape.
pub(crate) struct ClipAudio<S = TrackAudio> {
    source: S,
    /// Source frames from `window_start` on, kept so interpolation reaches across chunks.
    window: Vec<f32>,
    window_start: i64,
}

impl<S: FrameSource> ClipAudio<S> {
    pub(crate) fn new(source: S) -> Self {
        Self {
            source,
            window: Vec::new(),
            window_start: 0,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.source.reset();
        self.window.clear();
    }

    /// Adds `out.len() / 2` frames into `out`, the first `offset` frames into a clip
    /// that plays from source frame `source_in` at `speed`, all at the output rate.
    /// Frames before the media starts stay silent.
    pub(crate) fn mix_into(
        &mut self,
        media: &MediaItem,
        source_in: i64,
        offset: i64,
        speed: Speed,
        out: &mut [f32],
    ) -> Result<(), PlaybackError> {
        let frames = out.len() / CHANNELS;
        let (num, den) = (i128::from(speed.num()), i128::from(speed.den()));
        // The source frame playing at output frame `k`, and the fraction past it in `den`ths.
        let position = |k: usize| {
            let travelled = i128::from(offset + k as i64) * num;
            let whole = i128::from(source_in) + travelled.div_euclid(den);
            (
                i64::try_from(whole).unwrap_or(i64::MAX),
                travelled.rem_euclid(den),
            )
        };
        let Some(skip) = (0..frames).find(|&k| position(k).0 >= 0) else {
            return Ok(());
        };
        let out = &mut out[skip * CHANNELS..];
        let first = position(skip).0;
        if speed.is_normal() {
            return self.source.read(media, first, out);
        }
        let last = position(frames - 1).0;
        self.fill(media, (first - 1).max(0)..last + 3)?;
        for (k, pair) in (skip..frames).zip(out.as_chunks_mut::<CHANNELS>().0) {
            let (index, remainder) = position(k);
            let t = (remainder as f64 / den as f64) as f32;
            for (channel, sample) in pair.iter_mut().enumerate() {
                let at = |frame: i64| self.sample(frame, channel);
                *sample += cubic(at(index - 1), at(index), at(index + 1), at(index + 2), t);
            }
        }
        Ok(())
    }

    /// Clamped to the window, which repeats the edge frames where the media ends.
    fn sample(&self, frame: i64, channel: usize) -> f32 {
        let frames = (self.window.len() / CHANNELS) as i64;
        let index = (frame - self.window_start).clamp(0, (frames - 1).max(0));
        let index = usize::try_from(index).unwrap_or(0) * CHANNELS + channel;
        self.window.get(index).copied().unwrap_or(0.0)
    }

    /// Makes the window hold exactly `needed`, reading on from its end where it can so
    /// the source keeps decoding forward instead of seeking.
    fn fill(&mut self, media: &MediaItem, needed: Range<i64>) -> Result<(), PlaybackError> {
        let end = |window: &Self| window.window_start + (window.window.len() / CHANNELS) as i64;
        if needed.start < self.window_start || needed.start > end(self) || self.window.is_empty() {
            self.window.clear();
            self.window_start = needed.start;
        }
        let have = end(self);
        if needed.end > have {
            let old = self.window.len();
            let more = usize::try_from(needed.end - have).unwrap_or(0) * CHANNELS;
            self.window.resize(old + more, 0.0);
            self.source.read(media, have, &mut self.window[old..])?;
        }
        let stale = usize::try_from(needed.start - self.window_start).unwrap_or(0) * CHANNELS;
        self.window.drain(..stale.min(self.window.len()));
        self.window_start = needed.start;
        Ok(())
    }
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
    use timeline::{SourceId, Ticks};

    /// Each frame's value is its frame number, and every read's start is recorded.
    #[derive(Default)]
    struct Ramp {
        reads: Vec<i64>,
    }

    impl FrameSource for Ramp {
        fn read(
            &mut self,
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
            clip.mix_into(&media(), 100, chunk * 4, speed, &mut out)
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
        clip.mix_into(&media(), 0, -4, Speed::new(1, 2).unwrap(), &mut out)
            .unwrap();
        let left = left(&out);
        assert_eq!(left[..5], [0.0; 5]);
        assert!(left[5] > 0.0);
    }
}
