use std::ops::Range;

use crate::{Clip, Keyframe, Param, Ticks};

/// The spans of `levels`, one per `window` from `start`, at or above `threshold`, with
/// gaps shorter than `bridge` filled so the music doesn't bob between words.
pub fn active_spans(
    levels: &[f32],
    start: Ticks,
    window: Ticks,
    threshold: f32,
    bridge: Ticks,
) -> Vec<Range<Ticks>> {
    let mut spans: Vec<Range<Ticks>> = Vec::new();
    let at = |index: usize| start + Ticks(window.0 * index as i64);
    for (index, _) in levels
        .iter()
        .enumerate()
        .filter(|&(_, &level)| level >= threshold)
    {
        let span = at(index)..at(index + 1);
        match spans.last_mut() {
            Some(last) if span.start - last.end < bridge => last.end = span.end,
            _ => spans.push(span),
        }
    }
    spans
}

/// `clip`'s volume dipped by `depth` decibels through `spans` of sequence time, ramping
/// down over `fade` before each and back up over `fade` after; the clip's existing
/// keyframes are replaced. None when no span reaches the clip.
pub fn ducked_volume(
    clip: &Clip,
    spans: &[Range<Ticks>],
    depth: f32,
    fade: Ticks,
) -> Option<Param> {
    // Spans closer than two fades are joined so ramps never overlap.
    let mut joined: Vec<Range<Ticks>> = Vec::new();
    for span in spans {
        match joined.last_mut() {
            Some(last) if span.start - last.end <= fade + fade => last.end = last.end.max(span.end),
            _ => joined.push(span.clone()),
        }
    }
    let dip: Vec<(Ticks, f32)> = joined
        .iter()
        .flat_map(|span| {
            [
                (span.start - fade, 0.0),
                (span.start, depth),
                (span.end, depth),
                (span.end + fade, 0.0),
            ]
        })
        .collect();
    let dip_at = |time: Ticks| dip_at(&dip, time);
    let (first, last) = (clip.start, clip.end() - Ticks(1));
    let times = dip
        .iter()
        .map(|&(time, _)| time)
        .filter(|&time| first < time && time < last)
        .chain(
            [first, last]
                .into_iter()
                .filter(|&edge| dip_at(edge) != 0.0),
        );
    let base = clip.volume.value();
    let mut keys: Vec<Keyframe> = times
        .map(|time| {
            let Param { hundredths, .. } = Param::from_value(base - dip_at(time));
            Keyframe::new(clip.source_time(time), hundredths)
        })
        .collect();
    if keys.is_empty() {
        return None;
    }
    keys.sort_by_key(|key| key.time);
    keys.dedup_by_key(|key| key.time);
    Some(Param {
        hundredths: clip.volume.hundredths,
        keys,
    })
}

/// The dip at `time` along straight ramps between `points`, sorted by time.
fn dip_at(points: &[(Ticks, f32)], time: Ticks) -> f32 {
    let after = points.partition_point(|&(at, _)| at <= time);
    match (after.checked_sub(1).map(|i| points[i]), points.get(after)) {
        (Some((from, low)), Some(&(to, high))) => {
            let progress = (time - from).0 as f32 / (to - from).0.max(1) as f32;
            low + (high - low) * progress
        }
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClipId, SourceId};

    fn spans(ranges: &[(i64, i64)]) -> Vec<Range<Ticks>> {
        ranges
            .iter()
            .map(|&(start, end)| Ticks(start)..Ticks(end))
            .collect()
    }

    #[test]
    fn levels_above_the_threshold_join_across_short_gaps() {
        let levels = [-60.0, -20.0, -50.0, -20.0, -60.0, -60.0, -60.0, -10.0];
        let found = active_spans(&levels, Ticks(1_000), Ticks(10), -30.0, Ticks(20));
        assert_eq!(found, spans(&[(1_010, 1_040), (1_070, 1_080)]));
    }

    #[test]
    fn the_dip_ramps_around_each_span_in_source_time_and_is_clipped_to_the_clip() {
        let clip = Clip::new(
            ClipId(1),
            SourceId(1),
            Ticks(1_000),
            Ticks(1_000),
            Ticks(5_000),
        );
        // The first span starts inside the clip's head ramp; the second is close enough to
        // the third that the music stays down between them.
        let found = spans(&[
            (1_050, 1_200),
            (1_500, 1_600),
            (1_700, 1_800),
            (2_500, 2_600),
        ]);
        let volume = ducked_volume(&clip, &found, 18.0, Ticks(100)).unwrap();
        let keys: Vec<(i64, i32)> = volume
            .keys
            .iter()
            .map(|key| (key.time.0, key.hundredths))
            .collect();
        assert_eq!(
            keys,
            [
                (5_000, -900),
                (5_050, -1_800),
                (5_200, -1_800),
                (5_300, 0),
                (5_400, 0),
                (5_500, -1_800),
                (5_800, -1_800),
                (5_900, 0),
            ]
        );
        let elsewhere = spans(&[(3_000, 3_100)]);
        assert_eq!(ducked_volume(&clip, &elsewhere, 18.0, Ticks(100)), None);
    }
}
