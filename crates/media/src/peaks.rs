use std::ops::ControlFlow;
use std::path::Path;

use timeline::{Ticks, Timebase};

use crate::{AudioBuffer, DecodedFrame, MediaError, MediaReader, Streams};

/// One bucket per millisecond: finer than a frame at any rate, small enough to keep
/// hours of media in memory.
const PEAKS_PER_SECOND: u32 = 1000;
const PEAK_TIMEBASE: Timebase = match Timebase::new(1, PEAKS_PER_SECOND) {
    Some(timebase) => timebase,
    None => unreachable!(),
};
/// A corrupt timestamp far past the end must not allocate without bound.
const MAX_BUCKETS: i64 = PEAKS_PER_SECOND as i64 * 60 * 60 * 24;

/// The loudest sample of every millisecond of a file's audio, with coarser levels that
/// each halve the one before, so any span is measured from a handful of buckets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peaks {
    levels: Vec<Vec<u8>>,
}

impl Peaks {
    fn from_buckets(buckets: Vec<u8>) -> Self {
        let mut levels = vec![buckets];
        while let Some(finer) = levels.last().filter(|level| level.len() > 1) {
            let coarser = finer
                .chunks(2)
                .map(|pair| pair.iter().copied().max().unwrap_or(0))
                .collect();
            levels.push(coarser);
        }
        Self { levels }
    }

    /// The loudest absolute sample, from 0 to 1, in media time `start..end`; spans
    /// shorter than a bucket read the bucket they start in.
    pub fn max_in(&self, start: Ticks, end: Ticks, timebase: Timebase) -> f32 {
        let mut low = start.rescale(timebase, PEAK_TIMEBASE).0.max(0);
        let mut high = end.rescale(timebase, PEAK_TIMEBASE).0.max(low + 1);
        let mut peak = 0;
        // Climbs the levels, taking the odd bucket at either edge before halving the range,
        // so every bucket in the span is covered exactly once.
        for level in &self.levels {
            if low >= high {
                break;
            }
            let bucket = |index: i64| {
                usize::try_from(index)
                    .ok()
                    .and_then(|index| level.get(index))
                    .copied()
                    .unwrap_or(0)
            };
            if low & 1 == 1 {
                peak = peak.max(bucket(low));
                low += 1;
            }
            if high & 1 == 1 {
                high -= 1;
                peak = peak.max(bucket(high));
            }
            low >>= 1;
            high >>= 1;
        }
        f32::from(peak) / f32::from(u8::MAX)
    }
}

#[derive(Debug, Default)]
struct PeakBuilder {
    buckets: Vec<u8>,
}

impl PeakBuilder {
    fn add(&mut self, buffer: &AudioBuffer, timebase: Timebase) {
        let Some(sample_rate) = Timebase::new(1, buffer.sample_rate) else {
            return;
        };
        let channels = usize::from(buffer.channels.max(1));
        let first = buffer.pts.rescale(timebase, sample_rate).0;
        let rate = i64::from(buffer.sample_rate);
        for (offset, frame) in (0_i64..).zip(buffer.samples.chunks_exact(channels)) {
            let bucket = (first + offset).div_euclid(rate) * i64::from(PEAKS_PER_SECOND)
                + (first + offset).rem_euclid(rate) * i64::from(PEAKS_PER_SECOND) / rate;
            if !(0..MAX_BUCKETS).contains(&bucket) {
                continue;
            }
            let Ok(bucket) = usize::try_from(bucket) else {
                continue;
            };
            let loudest = frame
                .iter()
                .fold(0.0_f32, |peak, &sample| peak.max(sample.abs()));
            let level = quantize(loudest);
            if bucket >= self.buckets.len() {
                self.buckets.resize(bucket + 1, 0);
            }
            self.buckets[bucket] = self.buckets[bucket].max(level);
        }
    }

    fn finish(self) -> Peaks {
        Peaks::from_buckets(self.buckets)
    }
}

fn quantize(amplitude: f32) -> u8 {
    let scaled = (amplitude.clamp(0.0, 1.0) * f32::from(u8::MAX)).round();
    // In range after the clamp, so the cast cannot truncate.
    scaled as u8
}

/// Decodes all of `path`'s audio into peaks. `progress` hears the fraction done and can
/// stop the work. Blocks on decoding, so run it on a worker thread.
pub fn read_peaks(
    path: &Path,
    mut progress: impl FnMut(f32) -> ControlFlow<()>,
) -> Result<Peaks, MediaError> {
    let timebase = Timebase::HZ_90K;
    let mut reader = MediaReader::open_streams(path, timebase, Streams::Audio)?;
    let duration = reader.info().duration.filter(|duration| duration.0 > 0);
    let mut builder = PeakBuilder::default();
    while let Some(frame) = reader.next_frame()? {
        let DecodedFrame::Audio(buffer) = frame else {
            continue;
        };
        builder.add(&buffer, timebase);
        let done = duration.map_or(0.0, |duration| {
            (buffer.end(timebase).0 as f64 / duration.0 as f64) as f32
        });
        if progress(done.clamp(0.0, 1.0)).is_break() {
            return Err(MediaError::Cancelled);
        }
    }
    Ok(builder.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buffer(pts: Ticks, samples: Vec<f32>) -> AudioBuffer {
        AudioBuffer {
            pts,
            sample_rate: 48_000,
            channels: 2,
            samples,
        }
    }

    #[test]
    fn samples_land_in_the_millisecond_they_play_in() {
        let mut builder = PeakBuilder::default();
        // 2 ms of stereo starting 1 ms in: quiet left, a loud right sample in the second ms.
        let mut samples = vec![0.25; 2 * 96];
        samples[2 * 50 + 1] = -1.0;
        builder.add(&buffer(Ticks(90), samples), Timebase::HZ_90K);
        let peaks = builder.finish();
        let level = |ms: i64| {
            let at = |ms: i64| Ticks(ms * 90);
            peaks.max_in(at(ms), at(ms + 1), Timebase::HZ_90K)
        };
        assert_eq!(level(0), 0.0);
        assert_eq!(level(1), quantize(0.25) as f32 / 255.0);
        assert_eq!(level(2), 1.0);
        assert_eq!(level(3), 0.0);
    }

    #[test]
    fn a_long_span_finds_a_single_loud_bucket_through_the_coarse_levels() {
        let mut buckets = vec![10; 10_000];
        buckets[7_321] = 200;
        let peaks = Peaks::from_buckets(buckets);
        let ms = |ms: i64| Ticks(ms * 90);
        let span = |start, end| peaks.max_in(ms(start), ms(end), Timebase::HZ_90K);
        assert_eq!(span(7_000, 7_400), 200.0 / 255.0);
        assert_eq!(span(0, 10_000), 200.0 / 255.0);
        assert_eq!(span(7_330, 7_700), 10.0 / 255.0);
        assert_eq!(span(20_000, 21_000), 0.0);
    }
}
