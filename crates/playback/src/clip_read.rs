use timeline::Speed;

/// Where a clip's audio comes from, in frames at the output rate.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ClipRead<'a> {
    /// The first source frame of the clip's range, which a reversed clip plays last.
    pub source_in: i64,
    /// Output frames in the clip; a reversed clip counts back from its end.
    pub length: i64,
    pub speed: Speed,
    pub reverse: bool,
    pub maintain_pitch: bool,
    /// A time-remapped clip's speed keyframes as `[offset, source]` frames, which
    /// replace `speed` and `source_in` when there are at least two.
    pub curve: &'a [[i64; 2]],
}

impl ClipRead<'_> {
    /// The source position heard `frame` output frames into the clip, as a whole frame
    /// and the fraction past it.
    pub(crate) fn position(&self, frame: i64) -> (i64, f32) {
        if self.curve.len() >= 2 {
            // Beyond the first and last keyframes the nearest segment's speed carries on.
            let after = self.curve.partition_point(|&[offset, _]| offset <= frame);
            let after = after.clamp(1, self.curve.len() - 1);
            let ([from, source], [to, until]) = (self.curve[after - 1], self.curve[after]);
            return travel(source, frame - from, until - source, (to - from).max(1), 0);
        }
        let (num, den) = (self.speed.num(), self.speed.den());
        if self.reverse {
            travel(self.source_in, self.length - frame, num, den, 1)
        } else {
            travel(self.source_in, frame, num, den, 0)
        }
    }

    /// How many source frames pass each output frame at `frame`.
    pub(crate) fn speed_at(&self, frame: i64) -> f64 {
        if self.curve.len() >= 2 {
            let after = self.curve.partition_point(|&[offset, _]| offset <= frame);
            let after = after.clamp(1, self.curve.len() - 1);
            return segment_speed(self.curve[after - 1], self.curve[after]);
        }
        self.speed.num() as f64 / self.speed.den() as f64
    }

    pub(crate) fn fastest(&self) -> f64 {
        if self.curve.len() >= 2 {
            let speeds = self
                .curve
                .windows(2)
                .map(|pair| segment_speed(pair[0], pair[1]));
            return speeds.fold(0.0, f64::max);
        }
        self.speed_at(0)
    }

    /// Whether the source plays at anything but normal speed.
    pub(crate) fn is_varied(&self) -> bool {
        !self.speed.is_normal() || self.curve.len() >= 2
    }

    pub(crate) fn direction(&self) -> i64 {
        if self.reverse {
            -1
        } else {
            1
        }
    }
}

fn segment_speed([from, source]: [i64; 2], [to, until]: [i64; 2]) -> f64 {
    (until - source).abs() as f64 / (to - from).max(1) as f64
}

/// The source frame `frames * num / den` on from `from`, less `before`, and the fraction
/// past it.
fn travel(
    from: i64,
    frames: i64,
    num: impl Into<i128>,
    den: impl Into<i128>,
    before: i128,
) -> (i64, f32) {
    let (num, den) = (num.into(), den.into());
    let travelled = i128::from(frames) * num;
    let whole = i128::from(from) + travelled.div_euclid(den) - before;
    let fraction = travelled.rem_euclid(den) as f64 / den as f64;
    (i64::try_from(whole).unwrap_or(i64::MAX), fraction as f32)
}
