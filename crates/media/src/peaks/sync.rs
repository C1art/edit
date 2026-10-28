use std::ops::RangeInclusive;

use timeline::{Ticks, Timebase};

use super::{Peaks, PEAK_TIMEBASE};

/// The level searched first, of 32 ms buckets: coarse enough to try every lag quickly.
const COARSE_LEVEL: usize = 5;
/// The most of each file compared, ten minutes; cameras rolling together match well
/// before then.
const COMPARED_MS: usize = 600_000;
/// The least two files must overlap for a match to count.
const MIN_OVERLAP_MS: usize = 2_000;

impl Peaks {
    /// Where `other` starts in this file's time, found by lining up the sharp sounds the
    /// two recorded together, as when cameras rolled at different moments on one scene.
    /// None when either is silent or they don't overlap enough to compare.
    pub fn offset_of(&self, other: &Peaks, timebase: Timebase) -> Option<Ticks> {
        let level = |peaks: &Peaks, level: usize| {
            let buckets = peaks.levels.get(level)?;
            Some(onsets(buckets, COMPARED_MS >> level))
        };
        let (a, b) = (level(self, COARSE_LEVEL)?, level(other, COARSE_LEVEL)?);
        let overlap = (MIN_OVERLAP_MS >> COARSE_LEVEL) as i64;
        let lags = overlap - b.len() as i64..=a.len() as i64 - overlap;
        let coarse = best_lag(&a, &b, lags)?;
        // The coarse match is within a bucket either way; millisecond buckets settle it.
        let (a, b) = (level(self, 0)?, level(other, 0)?);
        let scale = 1 << COARSE_LEVEL;
        let fine = best_lag(&a, &b, (coarse - 1) * scale..=(coarse + 1) * scale)?;
        Some(Ticks(fine).rescale(PEAK_TIMEBASE, timebase))
    }
}

/// How much louder each bucket is than the one before, and zero where it gets quieter, so
/// claps and words stand out whatever the overall level of each recording.
fn onsets(buckets: &[u8], limit: usize) -> Vec<f32> {
    buckets[..buckets.len().min(limit)]
        .windows(2)
        .map(|pair| f32::from(pair[1].saturating_sub(pair[0])))
        .collect()
}

/// The lag at which `b[i]` best matches `a[i + lag]`; more onsets heard together score
/// higher, so a long overlap beats a lucky short one.
fn best_lag(a: &[f32], b: &[f32], lags: RangeInclusive<i64>) -> Option<i64> {
    lags.map(|lag| (lag, agreement(a, b, lag)))
        .filter(|&(_, score)| score > 0.0)
        .max_by(|x, y| x.1.total_cmp(&y.1))
        .map(|(lag, _)| lag)
}

fn agreement(a: &[f32], b: &[f32], lag: i64) -> f32 {
    let skip = |lag: i64| usize::try_from(lag).unwrap_or(0);
    let (a, b) = (
        a.get(skip(lag)..).unwrap_or_default(),
        b.get(skip(-lag)..).unwrap_or_default(),
    );
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minute of quiet room tone with a burst of sound every so often.
    fn scene() -> Vec<u8> {
        let mut state = 12_345_u32;
        (0..60_000)
            .map(|_| {
                state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                if state >> 24 < 4 {
                    200
                } else {
                    (state >> 28) as u8
                }
            })
            .collect()
    }

    #[test]
    fn a_camera_that_rolled_later_lines_up_to_the_millisecond() {
        let wide = scene();
        // The close-up started 7.381 s into the wide shot, recorded quieter, and ran on.
        let mut close: Vec<u8> = wide[7_381..].iter().map(|&level| level / 2).collect();
        close.extend(std::iter::repeat_n(3, 5_000));
        let (wide, close) = (Peaks::from_buckets(wide), Peaks::from_buckets(close));
        let ms = Timebase::new(1, 1_000).unwrap();
        assert_eq!(wide.offset_of(&close, ms), Some(Ticks(7_381)));
        assert_eq!(close.offset_of(&wide, ms), Some(Ticks(-7_381)));
    }
}
