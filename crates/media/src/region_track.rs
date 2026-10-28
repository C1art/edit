use std::ops::{ControlFlow, Range};
use std::path::Path;

use timeline::{MaskTracking, Similarity, Ticks, Timebase};

use crate::block_match::{Gray, Pyramid, HALF_BLOCK};
use crate::fit::{robust_fit, MIN_OUTLIER_PIXELS};
use crate::motion::analysis_size;
use crate::{DecodedFrame, MediaError, MediaReader, Streams};

/// Finer than the camera's analysis, as a tracked thing may fill little of the picture.
const ANALYSIS_WIDTH: u32 = 640;
/// Blocks matched in a region at most; a large one spaces them out.
const MAX_BLOCKS: f64 = 300.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackDirection {
    Forward,
    Backward,
}

/// What to follow: the picture inside `region` from the frame showing at `start`.
#[derive(Debug, Clone)]
pub struct RegionTrack {
    /// The footage to stay within.
    pub within: Range<Ticks>,
    pub start: Ticks,
    pub direction: TrackDirection,
    /// How many frames to follow it through, or to the edge of `within` if `None`.
    pub frames: Option<usize>,
    /// The region's outline, in fractions of the picture.
    pub region: Vec<[f64; 2]>,
    pub tracking: MaskTracking,
}

/// Where a tracked region is in one frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackedFrame {
    pub time: Ticks,
    /// Carries the region from the start frame to this one, in the square units of a
    /// picture of `size`.
    pub pose: Similarity,
    pub size: [u32; 2],
    /// The share of the frames to follow it through done.
    pub progress: f32,
}

/// Follows the picture inside a region through `path` frame by frame, handing each to
/// `found`, which can stop the work. Stops early where the region leaves the picture.
/// Blocks on decoding, so run it on a worker.
pub fn track_region(
    path: &Path,
    timebase: Timebase,
    track: &RegionTrack,
    mut found: impl FnMut(TrackedFrame) -> ControlFlow<()>,
) -> Result<(), MediaError> {
    let mut reader = MediaReader::open_streams(path, timebase, Streams::Video)?;
    let video = reader
        .info()
        .video
        .as_ref()
        .ok_or(MediaError::NoPlayableStream)?;
    let (width, height) = analysis_size(video.width, video.height, ANALYSIS_WIDTH);
    reader.set_video_size(Some((width, height)))?;
    reader.seek(track.start)?;
    let Some((start, first)) = next_picture(&mut reader)? else {
        return Ok(());
    };
    let mut frames = match track.direction {
        TrackDirection::Forward => Frames::Forward(reader),
        TrackDirection::Backward => Frames::Backward {
            half_second: Ticks(i64::from(timebase.den() / timebase.num().max(1)) / 2),
            reader,
            before: start,
            floor: track.within.start,
            pending: Vec::new(),
        },
    };
    let edge = match track.direction {
        TrackDirection::Forward => track.within.end - Ticks(1),
        TrackDirection::Backward => track.within.start,
    };
    let mut tracker = Tracker::new(Pyramid::new(first), &track.region, track.tracking);
    let mut count = 0;
    while track.frames.is_none_or(|limit| count < limit) {
        let Some((time, picture)) = frames.next()? else {
            break;
        };
        if !track.within.contains(&time) {
            break;
        }
        count += 1;
        let Some(pose) = tracker.follow(Pyramid::new(picture)) else {
            break;
        };
        let progress = match track.frames {
            Some(limit) => count as f32 / limit as f32,
            None => (time - start).0 as f32 / (edge - start).0.max(1) as f32,
        };
        let frame = TrackedFrame {
            time,
            pose,
            size: [width, height],
            progress: progress.clamp(0.0, 1.0),
        };
        if found(frame).is_break() {
            break;
        }
    }
    Ok(())
}

fn next_picture(reader: &mut MediaReader) -> Result<Option<(Ticks, Gray)>, MediaError> {
    while let Some(frame) = reader.next_frame()? {
        if let DecodedFrame::Video(frame) = frame {
            let picture = Gray::from_rgba(&frame.pixels, frame.width, frame.height);
            return Ok(Some((frame.pts, picture)));
        }
    }
    Ok(None)
}

/// The frames after the start frame in the order tracked. Footage only decodes forward,
/// so going backward decodes half a second before the frames left and hands those out
/// latest first.
enum Frames {
    Forward(MediaReader),
    Backward {
        reader: MediaReader,
        half_second: Ticks,
        /// The earliest frame handed out so far.
        before: Ticks,
        floor: Ticks,
        pending: Vec<(Ticks, Gray)>,
    },
}

impl Frames {
    fn next(&mut self) -> Result<Option<(Ticks, Gray)>, MediaError> {
        match self {
            Self::Forward(reader) => next_picture(reader),
            Self::Backward {
                reader,
                half_second,
                before,
                floor,
                pending,
            } => {
                if pending.is_empty() && *before > *floor {
                    reader.seek((*before - *half_second).max(*floor))?;
                    while let Some((time, picture)) = next_picture(reader)? {
                        if time >= *before {
                            break;
                        }
                        pending.push((time, picture));
                    }
                    if let Some((earliest, _)) = pending.first() {
                        *before = *earliest;
                    }
                }
                Ok(pending.pop())
            }
        }
    }
}

/// A region followed from frame to frame.
struct Tracker {
    previous: Pyramid,
    /// The region's outline in the square units of the last frame.
    region: Vec<[f64; 2]>,
    pose: Similarity,
    last_step: Similarity,
    tracking: MaskTracking,
}

impl Tracker {
    fn new(first: Pyramid, region: &[[f64; 2]], tracking: MaskTracking) -> Self {
        let [width, height] = first.size().map(f64::from);
        let unit = width.max(height);
        let region = region
            .iter()
            .map(|[x, y]| [(x - 0.5) * width / unit, (y - 0.5) * height / unit])
            .collect();
        Self {
            previous: first,
            region,
            pose: Similarity::IDENTITY,
            last_step: Similarity::IDENTITY,
            tracking,
        }
    }

    /// Where the region has gone in `next` since the first frame, unless it has left
    /// the picture.
    fn follow(&mut self, next: Pyramid) -> Option<Similarity> {
        let step = region_step(
            &self.previous,
            &next,
            &self.region,
            &self.last_step,
            self.tracking,
        )?;
        self.region = self.region.iter().map(|point| step.apply(*point)).collect();
        self.pose = self.pose.then(&step);
        self.last_step = step;
        self.previous = next;
        Some(self.pose)
    }
}

/// Where the picture inside `region`, in square units, went from `previous` to `next`,
/// looking first where `guess` would take it.
fn region_step(
    previous: &Pyramid,
    next: &Pyramid,
    region: &[[f64; 2]],
    guess: &Similarity,
    tracking: MaskTracking,
) -> Option<Similarity> {
    let size = previous.size().map(f64::from);
    let unit = size[0].max(size[1]);
    let to_pixels =
        |point: [f64; 2]| [0, 1].map(|axis| point[axis] * unit + size[axis] / 2.0 - 0.5);
    let to_square =
        |point: [f64; 2]| [0, 1].map(|axis| (point[axis] + 0.5 - size[axis] / 2.0) / unit);
    let outline: Vec<[f64; 2]> = region.iter().map(|point| to_pixels(*point)).collect();
    let margin = f64::from(HALF_BLOCK + 1);
    let low = [0, 1].map(|axis| {
        let least = outline
            .iter()
            .map(|point| point[axis])
            .fold(f64::INFINITY, f64::min);
        least.max(margin)
    });
    let high = [0, 1].map(|axis| {
        let most = outline
            .iter()
            .map(|point| point[axis])
            .fold(f64::NEG_INFINITY, f64::max);
        most.min(size[axis] - margin)
    });
    if low[0] >= high[0] || low[1] >= high[1] {
        return None;
    }
    let spacing = ((high[0] - low[0]) * (high[1] - low[1]) / MAX_BLOCKS)
        .sqrt()
        .max(f64::from(HALF_BLOCK));
    let count = |axis: usize| ((high[axis] - low[axis]) / spacing) as i32 + 1;
    let samples: Vec<[i32; 2]> = (0..count(1))
        .flat_map(|row| (0..count(0)).map(move |column| [column, row]))
        .map(|cell| [0, 1].map(|axis| low[axis] + f64::from(cell[axis]) * spacing))
        .filter(|point| inside(&outline, *point))
        .map(|point| point.map(|c| c.round() as i32))
        .collect();
    if samples.is_empty() {
        return None;
    }
    let middle = samples
        .iter()
        .fold([0.0, 0.0], |sum, point| {
            [0, 1].map(|axis| sum[axis] + f64::from(point[axis]))
        })
        .map(|sum| sum / samples.len() as f64);
    let expected = to_pixels(guess.apply(to_square(middle)));
    let expected = [0, 1].map(|axis| (expected[axis] - middle[axis]).round() as i32);
    let shift = previous.region_shift(next, &samples, expected);
    // A block straddling the outline matches the scene behind as much as the thing.
    let half = f64::from(HALF_BLOCK);
    let within = |center: &&[i32; 2]| {
        let [x, y] = center.map(f64::from);
        [[-half, -half], [half, -half], [half, half], [-half, half]]
            .iter()
            .all(|[dx, dy]| inside(&outline, [x + dx, y + dy]))
    };
    let interior: Vec<[i32; 2]> = samples.iter().filter(within).copied().collect();
    let blocks = if interior.is_empty() {
        &samples
    } else {
        &interior
    };
    let pairs: Vec<_> = previous
        .match_blocks(next, blocks.iter().copied(), shift)
        .into_iter()
        .map(|(from, to)| (to_square(from), to_square(to)))
        .collect();
    if pairs.is_empty() {
        return Some(Similarity {
            shift: shift.map(|d| f64::from(d) / unit),
            ..Similarity::IDENTITY
        });
    }
    Some(robust_fit(&pairs, MIN_OUTLIER_PIXELS / unit, tracking))
}

/// Whether `point` lies inside the polygon `outline`, by the crossings of a ray from it.
fn inside(outline: &[[f64; 2]], [x, y]: [f64; 2]) -> bool {
    let sides = outline.iter().zip(outline.iter().cycle().skip(1));
    sides
        .filter(|([_, ay], [_, by])| (*ay > y) != (*by > y))
        .filter(|([ax, ay], [bx, by])| x < ax + (y - ay) / (by - ay) * (bx - ax))
        .count()
        % 2
        == 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_picture::{draw, texture};

    #[test]
    fn a_textured_disk_is_followed_as_it_moves_turns_and_grows_over_a_still_scene() {
        let step = Similarity {
            shift: [0.012, 0.006],
            angle: 0.03,
            scale: 1.02,
        };
        let (center, radius) = ([-0.15, 0.0], 0.12);
        let frame = |pose: &Similarity| {
            let back = pose.inverse();
            draw(|point| {
                let [x, y] = back.apply(point);
                if (x - center[0]).hypot(y - center[1]) < radius {
                    texture(x * 320.0, y * 320.0)
                } else {
                    texture(point[0] * 320.0 + 1_000.0, point[1] * 320.0)
                }
            })
        };
        let region: Vec<[f64; 2]> = (0..24)
            .map(|index| {
                let angle = std::f64::consts::TAU * f64::from(index) / 24.0;
                let [x, y] =
                    [0, 1].map(|axis| center[axis] + radius * [angle.cos(), angle.sin()][axis]);
                [x + 0.5, y * 320.0 / 180.0 + 0.5]
            })
            .collect();
        let mut pose = Similarity::IDENTITY;
        let mut tracker = Tracker::new(frame(&pose), &region, MaskTracking::PositionScaleRotation);
        let mut found = Similarity::IDENTITY;
        for _ in 0..4 {
            pose = pose.then(&step);
            found = tracker.follow(frame(&pose)).unwrap();
        }
        let off = (found.shift[0] - pose.shift[0]).abs() + (found.shift[1] - pose.shift[1]).abs();
        assert!(off < 3e-3, "{found:?} is not {pose:?}");
        assert!(
            (found.angle - pose.angle).abs() < 6e-3,
            "{found:?} is not {pose:?}"
        );
        assert!(
            (found.scale - pose.scale).abs() < 6e-3,
            "{found:?} is not {pose:?}"
        );

        let mut shift_only = Tracker::new(
            frame(&Similarity::IDENTITY),
            &region,
            MaskTracking::Position,
        );
        let moved = shift_only.follow(frame(&step)).unwrap();
        assert!(moved.angle == 0.0 && moved.scale == 1.0, "{moved:?}");
    }
}
