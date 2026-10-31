//! Where the beats of a piece of music fall, found from how loud it is through time:
//! the tempo from how its jumps in loudness repeat, then the run of beats that keeps
//! closest to that tempo while landing on the strongest jumps (Ellis's beat tracker).

use crate::{Ticks, Timebase};

const SLOWEST_BPM: f32 = 60.0;
const FASTEST_BPM: f32 = 200.0;
/// Tempos are taken as most likely near this, so a song isn't heard at double or half
/// its speed when both fit.
const LIKELIEST_BPM: f32 = 120.0;
/// How many octaves from the likeliest tempo still count as likely.
const TEMPO_SPREAD: f32 = 1.0;
/// How strongly beats keep to the tempo rather than chase every loud jump.
const TIGHTNESS: f32 = 100.0;
/// Beats at the ends with less of a hit than this share of the typical beat's carry on
/// the pulse into an intro or outro without anything playing it.
const UNHEARD: f32 = 0.5;
/// Jumps from below this under the loudest part are noise, not hits.
const FLOOR_DB: f32 = 80.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Beats {
    pub per_minute: f32,
    pub at: Vec<Ticks>,
}

/// The beats in `levels`, decibels one per `window` from `start` in `timebase`;
/// `None` when the music is too short or too quiet to hear a steady pulse in.
pub fn beats(levels: &[f32], (start, window): (Ticks, Ticks), timebase: Timebase) -> Option<Beats> {
    let seconds = Timebase::new(1, 1)?;
    let windows_per_minute = Ticks(60).rescale(seconds, timebase).0 as f32 / window.0 as f32;
    let loudest = levels.iter().copied().fold(f32::MIN, f32::max);
    let onsets = onset_strength(levels, loudest - FLOOR_DB);
    let period = tempo_period(&onsets, windows_per_minute)?;
    let tracked = track(&onsets, period);
    let hits: Vec<f32> = tracked
        .iter()
        .map(|&beat| {
            let near = beat.saturating_sub(1)..(beat + 2).min(onsets.len());
            onsets[near].iter().copied().fold(0.0, f32::max)
        })
        .collect();
    let mut sorted = hits.clone();
    sorted.sort_unstable_by(f32::total_cmp);
    let typical = sorted.get(sorted.len() / 2).copied().unwrap_or_default();
    let heard = |hit: &f32| *hit > typical * UNHEARD;
    let first = hits.iter().position(heard)?;
    let last = hits.iter().rposition(heard)?;
    let at = tracked[first..=last]
        .iter()
        .map(|&beat| start + Ticks(window.0 * beat as i64))
        .collect();
    Some(Beats {
        per_minute: windows_per_minute / period,
        at,
    })
}

/// How much louder each window is than the one before, scaled to unit spread.
fn onset_strength(levels: &[f32], floor: f32) -> Vec<f32> {
    let rises: Vec<f32> = std::iter::once(0.0)
        .chain(
            levels
                .windows(2)
                .map(|pair| (pair[1].max(floor) - pair[0].max(floor)).max(0.0)),
        )
        .collect();
    let count = rises.len().max(1) as f32;
    let mean = rises.iter().sum::<f32>() / count;
    let spread = (rises.iter().map(|rise| (rise - mean).powi(2)).sum::<f32>() / count).sqrt();
    if spread <= f32::EPSILON {
        return rises;
    }
    rises.into_iter().map(|rise| rise / spread).collect()
}

/// The beat period in windows: the lag at which the onsets best repeat, weighted
/// toward likely tempos.
fn tempo_period(onsets: &[f32], windows_per_minute: f32) -> Option<f32> {
    let shortest = (windows_per_minute / FASTEST_BPM).floor().max(1.0) as usize;
    let longest = (windows_per_minute / SLOWEST_BPM).ceil() as usize;
    if onsets.len() < longest * 4 {
        return None;
    }
    let correlation = |lag: usize| {
        let sum: f32 = onsets.iter().zip(&onsets[lag..]).map(|(a, b)| a * b).sum();
        sum / (onsets.len() - lag) as f32
    };
    let likelihood = |lag: usize| {
        let octaves = (windows_per_minute / lag as f32 / LIKELIEST_BPM).log2() / TEMPO_SPREAD;
        (-0.5 * octaves * octaves).exp()
    };
    let scores: Vec<f32> = (shortest..=longest)
        .map(|lag| correlation(lag) * likelihood(lag))
        .collect();
    let (best, &peak) = scores
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))?;
    if peak <= 0.0 {
        return None;
    }
    // Between windows, from the scores either side of the best.
    let shift = match (best.checked_sub(1), scores.get(best + 1)) {
        (Some(before), Some(&after)) => {
            let before = scores[before];
            let curve = before - 2.0 * peak + after;
            if curve < 0.0 {
                (0.5 * (before - after) / curve).clamp(-0.5, 0.5)
            } else {
                0.0
            }
        }
        _ => 0.0,
    };
    Some((shortest + best) as f32 + shift)
}

/// The windows of the run of beats about `period` apart that best follows the onsets.
fn track(onsets: &[f32], period: f32) -> Vec<usize> {
    let mut scores = Vec::with_capacity(onsets.len());
    let mut previous: Vec<Option<usize>> = Vec::with_capacity(onsets.len());
    let (nearest, farthest) = (
        (period / 2.0).round().max(1.0) as usize,
        (period * 2.0).round() as usize,
    );
    for (index, &onset) in onsets.iter().enumerate() {
        let best = (index.saturating_sub(farthest)..index.saturating_sub(nearest - 1))
            .map(|before| {
                let stretch = ((index - before) as f32 / period).ln();
                (before, scores[before] - TIGHTNESS * stretch * stretch)
            })
            .max_by(|a: &(usize, f32), b| a.1.total_cmp(&b.1));
        let (link, carried) = best.map_or((None, 0.0), |(before, score)| (Some(before), score));
        scores.push(onset + carried.max(0.0));
        previous.push(link.filter(|_| carried > 0.0));
    }
    // The last beat is the best scoring one within a period of the end.
    let tail = onsets.len().saturating_sub(period.ceil() as usize);
    let Some(mut beat) = (tail..onsets.len()).max_by(|&a, &b| scores[a].total_cmp(&scores[b]))
    else {
        return Vec::new();
    };
    let mut beats = vec![beat];
    while let Some(before) = previous[beat] {
        beats.push(before);
        beat = before;
    }
    beats.reverse();
    beats
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 100 windows a second: a quiet bed with a hit every half second from `first`, and
    /// softer hits halfway between.
    fn drums(first: usize, length: usize) -> Vec<f32> {
        (0..length)
            .map(|index| match (index + 50 - first % 50) % 50 {
                0..=2 if index >= first => -6.0,
                25..=26 if index >= first => -18.0,
                _ => -30.0 + (index % 7) as f32 * 0.3,
            })
            .collect()
    }

    #[test]
    fn beats_land_on_the_hits_at_their_tempo() {
        let window = Ticks(900);
        let found = beats(&drums(13, 3_000), (Ticks(90_000), window), Timebase::HZ_90K).unwrap();
        assert!(
            (found.per_minute - 120.0).abs() < 1.0,
            "{}",
            found.per_minute
        );
        assert_eq!(found.at.len(), 60);
        for (count, at) in found.at.iter().enumerate() {
            let hit = Ticks(90_000) + Ticks(900 * (13 + 50 * count as i64));
            assert!((at.0 - hit.0).abs() <= 900, "beat {count} at {at:?}");
        }
    }

    #[test]
    fn silence_before_the_music_has_no_beats() {
        let mut levels = vec![-120.0; 500];
        levels.extend(drums(0, 2_000));
        let found = beats(&levels, (Ticks::ZERO, Ticks(900)), Timebase::HZ_90K).unwrap();
        assert!(found.at[0] >= Ticks(900 * 499), "{:?}", found.at[0]);
        assert!(beats(
            &[-120.0; 2_000],
            (Ticks::ZERO, Ticks(900)),
            Timebase::HZ_90K
        )
        .is_none());
    }
}
