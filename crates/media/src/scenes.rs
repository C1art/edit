use std::ops::{ControlFlow, Range};
use std::path::Path;

use timeline::{Ticks, Timebase};

use crate::{DecodedFrame, MediaError, MediaReader, Streams};

/// Small enough to decode quickly, large enough that a new shot changes its colors.
const ANALYSIS_SIZE: (u32, u32) = (64, 36);
const BINS: usize = 16;
/// The least share of the colors a cut changes.
const MIN_CHANGE: f32 = 0.3;
/// How many times the change around it a cut's change is, so fast motion isn't a cut.
const STANDOUT: f32 = 3.0;
/// Frames either side that a change is measured against.
const NEIGHBORS: usize = 5;
/// The shortest shot, in milliseconds; a flash frame isn't a shot of its own.
const SHORTEST_SHOT_MS: i64 = 500;
const MILLISECONDS: Timebase = match Timebase::new(1, 1000) {
    Some(timebase) => timebase,
    None => unreachable!(),
};

/// Each channel's histogram of a picture, as shares of its pixels.
struct Colors([f32; 3 * BINS]);

impl Colors {
    fn of(rgba: &[u8]) -> Self {
        let mut counts = [0.0; 3 * BINS];
        let (pixels, _) = rgba.as_chunks::<4>();
        let total = pixels.len().max(1) as f32;
        for pixel in pixels {
            for (channel, &value) in pixel[..3].iter().enumerate() {
                counts[channel * BINS + usize::from(value) * BINS / 256] += 1.0;
            }
        }
        Self(counts.map(|count| count / total))
    }

    /// From 0 (the same colors) to 1 (none in common).
    fn change_to(&self, other: &Self) -> f32 {
        let moved: f32 = self
            .0
            .iter()
            .zip(&other.0)
            .map(|(a, b)| (a - b).abs())
            .sum();
        // Each channel's shares sum to 1, so at most 2 of them move per channel.
        moved / 6.0
    }
}

/// Where new shots start in `range` of `path`'s picture, as the times of their first
/// frames. `progress` hears the fraction done and can stop the work. Blocks on decoding,
/// so run it on a worker thread.
pub fn detect_scenes(
    path: &Path,
    timebase: Timebase,
    range: Range<Ticks>,
    mut progress: impl FnMut(f32) -> ControlFlow<()>,
) -> Result<Vec<Ticks>, MediaError> {
    let mut reader = MediaReader::open_streams(path, timebase, Streams::Video)?;
    reader.set_video_size(Some(ANALYSIS_SIZE))?;
    reader.seek(range.start)?;
    let span = (range.end - range.start).0.max(1) as f32;
    let mut previous = None;
    let mut changes = Vec::new();
    while let Some(frame) = reader.next_frame()? {
        let DecodedFrame::Video(frame) = frame else {
            continue;
        };
        if frame.pts >= range.end {
            break;
        }
        let colors = Colors::of(&frame.pixels);
        if let Some(previous) = &previous {
            changes.push((frame.pts, Colors::change_to(previous, &colors)));
        }
        previous = Some(colors);
        let done = (frame.pts - range.start).0 as f32 / span;
        if progress(done.clamp(0.0, 1.0)).is_break() {
            return Err(MediaError::Cancelled);
        }
    }
    let shortest = Ticks(SHORTEST_SHOT_MS).rescale(MILLISECONDS, timebase);
    Ok(cuts(&changes, shortest))
}

/// The frames that start new shots, from each frame's time and how much its colors
/// changed from the frame before. Of cuts closer than `shortest`, the larger stays.
fn cuts(changes: &[(Ticks, f32)], shortest: Ticks) -> Vec<Ticks> {
    let mut found: Vec<(Ticks, f32)> = changes
        .iter()
        .enumerate()
        .filter(|&(index, &(_, change))| {
            let around =
                index.saturating_sub(NEIGHBORS)..(index + NEIGHBORS + 1).min(changes.len());
            let others: Vec<f32> = changes[around.clone()]
                .iter()
                .zip(around)
                .filter(|&(_, at)| at != index)
                .map(|(&(_, change), _)| change)
                .collect();
            let usual = others.iter().sum::<f32>() / others.len().max(1) as f32;
            change >= MIN_CHANGE && change >= STANDOUT * usual
        })
        .map(|(_, &cut)| cut)
        .collect();
    found.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut kept: Vec<Ticks> = Vec::new();
    for (at, _) in found {
        if kept.iter().all(|&cut| (cut - at).0.abs() >= shortest.0) {
            kept.push(at);
        }
    }
    kept.sort();
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames(changes: &[f32]) -> Vec<(Ticks, f32)> {
        (1..).map(Ticks).zip(changes.iter().copied()).collect()
    }

    #[test]
    fn a_cut_stands_out_from_the_motion_around_it() {
        let mut changes = vec![0.05; 30];
        changes[9] = 0.8;
        // Fast motion changes a lot every frame, so no frame of it stands out.
        changes[20..].fill(0.35);
        assert_eq!(cuts(&frames(&changes), Ticks(5)), [Ticks(10)]);
    }

    #[test]
    fn of_two_cuts_closer_than_the_shortest_shot_the_larger_stays() {
        let mut changes = vec![0.02; 40];
        changes[9] = 0.6;
        changes[11] = 0.9;
        changes[29] = 0.5;
        assert_eq!(cuts(&frames(&changes), Ticks(5)), [Ticks(12), Ticks(30)]);
    }

    #[test]
    #[ignore = "decodes a real file with FFmpeg"]
    fn shots_start_on_the_first_frame_of_each_shot_in_the_range_read() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/cuts_1s_1.8s_25fps.mp4");
        let seconds = |s: f64| Ticks((s * 90_000.0).round() as i64);
        let detect = |range| {
            detect_scenes(
                &path,
                Timebase::HZ_90K,
                range,
                |_| ControlFlow::Continue(()),
            )
            .unwrap()
        };
        assert_eq!(
            detect(Ticks::ZERO..seconds(2.8)),
            [seconds(1.0), seconds(1.8)]
        );
        assert_eq!(detect(seconds(1.2)..seconds(2.8)), [seconds(1.8)]);
    }

    #[test]
    fn colors_change_fully_between_pictures_with_none_in_common() {
        let black = Colors::of(&[0, 0, 0, 255].repeat(4));
        let white = Colors::of(&[255, 255, 255, 255].repeat(4));
        assert_eq!(black.change_to(&white), 1.0);
        assert_eq!(black.change_to(&black), 0.0);
    }
}
