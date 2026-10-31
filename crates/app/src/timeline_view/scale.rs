use timeline::{ClipId, FrameRate, Sequence, Ticks, Timebase};

/// Maps sequence time to horizontal screen position. Floating point appears only here,
/// at the display boundary.
#[derive(Debug, Clone, Copy)]
pub struct TimeScale {
    /// Screen x of `scroll`.
    pub left: f32,
    pub scroll: Ticks,
    pub ticks_per_point: f64,
}

impl TimeScale {
    pub fn x(&self, time: Ticks) -> f32 {
        self.left + ((time.0 - self.scroll.0) as f64 / self.ticks_per_point) as f32
    }

    pub fn time(&self, x: f32) -> Ticks {
        self.scroll + self.span(x - self.left)
    }

    pub fn span(&self, points: f32) -> Ticks {
        Ticks((f64::from(points) * self.ticks_per_point).round() as i64)
    }
}

/// The start of the frame nearest to `time`.
pub fn nearest_frame(time: Ticks, rate: FrameRate, timebase: Timebase) -> Ticks {
    let frame = rate.frame_at(time, timebase);
    let start = rate.frame_start(frame, timebase);
    let next = rate.frame_start(frame + 1, timebase);
    if time - start < next - time {
        start
    } else {
        next
    }
}

/// Ruler label spacing in frames: the smallest "round" interval at least `min_points`
/// wide.
pub fn label_interval(
    scale: &TimeScale,
    rate: FrameRate,
    timebase: Timebase,
    min_points: f32,
) -> i64 {
    let fps = rate.num().div_ceil(rate.den()).max(1);
    let fps = i64::from(fps);
    let candidates = [1, 2, 5, 10]
        .into_iter()
        .chain([1, 2, 5, 10, 15, 30, 60, 120, 300, 600, 1800, 3600].map(|seconds| seconds * fps));
    let frame_points = |frames: i64| {
        let ticks = rate.frame_start(frames, timebase);
        (ticks.0 as f64 / scale.ticks_per_point) as f32
    };
    candidates
        .filter(|&frames| frames < fps || frames % fps == 0)
        .find(|&frames| frame_points(frames) >= min_points)
        .unwrap_or(3600 * fps)
}

/// Moves `delta` so that one of `edges` lands on the closest of `targets`, when one is
/// within `threshold`.
pub fn snap(delta: Ticks, edges: &[Ticks], targets: &[Ticks], threshold: Ticks) -> Ticks {
    edges
        .iter()
        .flat_map(|&edge| targets.iter().map(move |&target| target - (edge + delta)))
        .filter(|offset| offset.0.abs() <= threshold.0)
        .min_by_key(|offset| offset.0.abs())
        .map_or(delta, |offset| delta + offset)
}

/// Where moved or dropped clips snap to: the edges and markers of every clip but
/// `moving`, the sequence's markers, the playhead and the start.
pub fn snap_targets(sequence: &Sequence, moving: &[ClipId], playhead: Ticks) -> Vec<Ticks> {
    let (rate, timebase) = (sequence.format.frame_rate, sequence.timebase);
    let mut targets: Vec<Ticks> = sequence
        .tracks()
        .flat_map(|(_, track)| &track.clips)
        .filter(|clip| !moving.contains(&clip.id))
        .flat_map(|clip| {
            // A marker on a sped-up or reversed clip can fall between frames, where a
            // clip snapped to it would start part-way through one.
            let marks = clip
                .shown_markers()
                .map(|(position, _)| nearest_frame(position, rate, timebase));
            [clip.start, clip.end()].into_iter().chain(marks)
        })
        .collect();
    targets.extend(sequence.markers.iter().map(|marker| marker.at));
    targets.push(playhead);
    targets.push(Ticks::ZERO);
    targets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_picks_the_closest_target_within_threshold() {
        let edges = [Ticks(100), Ticks(500)];
        let targets = [Ticks(130), Ticks(495), Ticks(1000)];
        assert_eq!(snap(Ticks(10), &edges, &targets, Ticks(20)), Ticks(-5));
        assert_eq!(
            snap(Ticks(10), &edges, &[Ticks(1000)], Ticks(20)),
            Ticks(10)
        );
    }
}
