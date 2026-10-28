use serde::{Deserialize, Serialize};

use crate::keyframe::{HandleSide, Handles, Interpolation, Keyframe};
use crate::Ticks;

/// An adjustable clip property, held in hundredths of its display unit so the model
/// stays `Eq` and round-trips exactly. With keyframes it is animated, moving between
/// them as each one's interpolation says and holding the first and last beyond them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Param {
    /// The value when there are no keyframes.
    pub hundredths: i32,
    /// Sorted by time, without duplicates.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<Keyframe>,
}

impl Param {
    pub const fn new(hundredths: i32) -> Self {
        Self {
            hundredths,
            keys: Vec::new(),
        }
    }

    pub fn from_value(value: f32) -> Self {
        Self::new(to_hundredths(value))
    }

    /// The value without animation.
    pub fn value(&self) -> f32 {
        self.hundredths as f32 / 100.0
    }

    pub fn is_animated(&self) -> bool {
        !self.keys.is_empty()
    }

    pub fn value_at(&self, time: Ticks) -> f32 {
        let after = self.keys.partition_point(|key| key.time <= time);
        let hundredths = match (
            after.checked_sub(1).map(|i| self.keys[i]),
            self.keys.get(after).copied(),
        ) {
            (None, None) => f64::from(self.hundredths),
            (Some(key), None) | (None, Some(key)) => f64::from(key.hundredths),
            (Some(before), Some(_)) if before.interpolation == Interpolation::Hold => {
                f64::from(before.hundredths)
            }
            (Some(before), Some(next)) => {
                let span = (next.time - before.time).0 as f64;
                let (from, to) = (f64::from(before.hundredths), f64::from(next.hundredths));
                let straight = (to - from) / span;
                let slope = |index: usize, eased: bool, side: HandleSide| {
                    let key = self.keys[index];
                    match (key.interpolation, key.handles) {
                        _ if eased => 0.0,
                        (interpolation, Some(handles)) if interpolation.has_handles() => {
                            handles.slope(side)
                        }
                        (interpolation, _)
                            if interpolation == Interpolation::AutoBezier
                                || interpolation.has_handles() =>
                        {
                            self.smooth_slope(index).unwrap_or(straight)
                        }
                        _ => straight,
                    }
                };
                let leaving = slope(
                    after - 1,
                    before.interpolation.eases_out(),
                    HandleSide::Outgoing,
                );
                let arriving = slope(after, next.interpolation.eases_in(), HandleSide::Incoming);
                hermite(
                    (time - before.time).0 as f64 / span,
                    [from, to],
                    [leaving * span, arriving * span],
                )
            }
        };
        (hundredths / 100.0) as f32
    }

    /// The hundredths per tick an auto Bezier curve passes the key at `index` with: along
    /// the line between its neighbours, but level at a peak or dip so the curve never
    /// overshoots a keyed value. `None` at the first or last key.
    fn smooth_slope(&self, index: usize) -> Option<f64> {
        let [previous, key, next] = [index.checked_sub(1)?, index, index + 1]
            .map(|index| self.keys.get(index))
            .map(|key| key.map(|key| (key.time.0 as f64, f64::from(key.hundredths))));
        let ((previous_time, previous), (_, value), (next_time, next)) = (previous?, key?, next?);
        let turns = (value - previous) * (next - value) <= 0.0;
        Some(if turns {
            0.0
        } else {
            (next - previous) / (next_time - previous_time)
        })
    }

    /// Sets the value at `time`: a keyframe there when animated, the whole value otherwise.
    pub fn set_at(&mut self, time: Ticks, value: f32) {
        let hundredths = to_hundredths(value);
        if !self.is_animated() {
            self.hundredths = hundredths;
            return;
        }
        match self.keys.binary_search_by_key(&time, |key| key.time) {
            Ok(index) => self.keys[index].hundredths = hundredths,
            Err(index) => self.keys.insert(index, Keyframe::new(time, hundredths)),
        }
    }

    /// Starts animating with a keyframe at `time`, or stops and keeps the value there.
    pub fn toggle_animation(&mut self, time: Ticks) {
        let hundredths = to_hundredths(self.value_at(time));
        if self.is_animated() {
            self.keys.clear();
            self.hundredths = hundredths;
        } else {
            self.keys.push(Keyframe::new(time, hundredths));
        }
    }

    pub fn has_key_at(&self, time: Ticks) -> bool {
        self.keys.iter().any(|key| key.time == time)
    }

    /// Adds a keyframe at `time` holding the current value, or removes the one there.
    /// Removing the last keyframe stops the animation at its value.
    pub fn toggle_key(&mut self, time: Ticks) {
        let value = self.value_at(time);
        match self.keys.binary_search_by_key(&time, |key| key.time) {
            Ok(index) => {
                let removed = self.keys.remove(index);
                if self.keys.is_empty() {
                    self.hundredths = removed.hundredths;
                }
            }
            Err(index) => self
                .keys
                .insert(index, Keyframe::new(time, to_hundredths(value))),
        }
    }

    /// Retimes the keyframe at `from` to `to`, passing others if need be; `false` when
    /// there is none at `from` or another already at `to`.
    pub fn move_key(&mut self, from: Ticks, to: Ticks) -> bool {
        if from == to || self.has_key_at(to) {
            return false;
        }
        let Some(index) = self.keys.iter().position(|key| key.time == from) else {
            return false;
        };
        let mut key = self.keys.remove(index);
        key.time = to;
        let index = self.keys.partition_point(|other| other.time < to);
        self.keys.insert(index, key);
        true
    }

    /// Adds `key`, replacing any keyframe already at its time.
    pub fn put_key(&mut self, key: Keyframe) {
        match self.keys.binary_search_by_key(&key.time, |kept| kept.time) {
            Ok(index) => self.keys[index] = key,
            Err(index) => self.keys.insert(index, key),
        }
    }

    /// Moves the keyframes at `times` together by `by`; returns false, moving none, if
    /// one would land on a keyframe that stays put or before the source's start.
    pub fn shift_keys(&mut self, times: &[Ticks], by: Ticks) -> bool {
        let moving = |key: &Keyframe| times.contains(&key.time);
        let blocked = self.keys.iter().filter(|key| moving(key)).any(|key| {
            let to = key.time + by;
            to < Ticks::ZERO
                || self
                    .keys
                    .iter()
                    .any(|kept| !moving(kept) && kept.time == to)
        });
        if by == Ticks::ZERO || blocked || !self.keys.iter().any(moving) {
            return false;
        }
        for key in self.keys.iter_mut().filter(|key| times.contains(&key.time)) {
            key.time = key.time + by;
        }
        self.keys.sort_by_key(|key| key.time);
        true
    }

    pub fn interpolation_at(&self, time: Ticks) -> Option<Interpolation> {
        self.keys
            .iter()
            .find(|key| key.time == time)
            .map(|key| key.interpolation)
    }

    /// Returns whether there was a keyframe at `time` to change. Bezier handles start
    /// along the curve's smooth path through the keyframe and join when made continuous.
    pub fn set_interpolation(&mut self, time: Ticks, interpolation: Interpolation) -> bool {
        let Some(index) = self.keys.iter().position(|key| key.time == time) else {
            return false;
        };
        let handles = match (interpolation, self.keys[index].handles) {
            (Interpolation::Bezier, Some(handles)) => Some(handles),
            (Interpolation::ContinuousBezier, Some(handles)) => Some(Handles::level(
                (handles.slope(HandleSide::Incoming) + handles.slope(HandleSide::Outgoing)) / 2.0,
            )),
            (interpolation, _) if interpolation.has_handles() => {
                Some(Handles::level(self.natural_slope(index)))
            }
            _ => None,
        };
        let key = &mut self.keys[index];
        key.interpolation = interpolation;
        key.handles = handles;
        true
    }

    /// Sets the slope, in hundredths per tick, of one handle of the Bezier keyframe at
    /// `time`, or both when it is continuous. Returns whether there was one to change.
    pub fn set_handle(&mut self, time: Ticks, side: HandleSide, slope: f64) -> bool {
        let key = self.keys.iter_mut().find(|key| key.time == time);
        let Some(key) = key.filter(|key| key.interpolation.has_handles()) else {
            return false;
        };
        let handles = key.handles.get_or_insert_with(Handles::default);
        match key.interpolation {
            Interpolation::ContinuousBezier => *handles = Handles::level(slope),
            _ => handles.set(side, slope),
        }
        true
    }

    /// The slope an untouched Bezier keyframe passes through with: the auto Bezier slope
    /// between neighbours, toward the only neighbour at either end.
    fn natural_slope(&self, index: usize) -> f64 {
        self.smooth_slope(index).unwrap_or_else(|| {
            let key = self.keys[index];
            let neighbour = index
                .checked_sub(1)
                .and_then(|previous| self.keys.get(previous))
                .or_else(|| self.keys.get(index + 1));
            neighbour.map_or(0.0, |other| {
                f64::from(other.hundredths - key.hundredths) / (other.time - key.time).0 as f64
            })
        })
    }

    pub fn previous_key(&self, time: Ticks) -> Option<Ticks> {
        self.keys
            .iter()
            .rev()
            .map(|key| key.time)
            .find(|&key| key < time)
    }

    pub fn next_key(&self, time: Ticks) -> Option<Ticks> {
        self.keys.iter().map(|key| key.time).find(|&key| key > time)
    }
}

/// The value `progress` through a segment between `values`, on the cubic Hermite curve
/// leaving and arriving with `slopes` given per whole segment, so slopes equal to the
/// change between the values draw a straight line.
fn hermite(progress: f64, [from, to]: [f64; 2], [leaving, arriving]: [f64; 2]) -> f64 {
    let p = progress;
    let (p2, p3) = (p * p, p * p * p);
    (2.0 * p3 - 3.0 * p2 + 1.0) * from
        + (p3 - 2.0 * p2 + p) * leaving
        + (3.0 * p2 - 2.0 * p3) * to
        + (p3 - p2) * arriving
}

fn to_hundredths(value: f32) -> i32 {
    (value * 100.0).round() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn animated_values_interpolate_between_keys_and_hold_outside() {
        let mut param = Param::from_value(50.0);
        param.toggle_animation(Ticks(100));
        param.set_at(Ticks(300), 150.0);
        assert_eq!(param.value_at(Ticks(0)), 50.0);
        assert_eq!(param.value_at(Ticks(150)), 75.0);
        assert_eq!(param.value_at(Ticks(400)), 150.0);

        // Editing between keys adds one; removing it restores the straight line.
        param.set_at(Ticks(200), 0.0);
        assert_eq!(param.value_at(Ticks(250)), 75.0);
        param.toggle_key(Ticks(200));
        assert_eq!(param.value_at(Ticks(200)), 100.0);
    }

    #[test]
    fn shifted_keys_move_together_unless_one_would_land_on_another() {
        let mut param = Param::from_value(0.0);
        for (time, value) in [(0, 0.0), (100, 10.0), (200, 20.0), (300, 30.0)] {
            param.put_key(Keyframe::new(Ticks(time), to_hundredths(value)));
        }
        assert!(!param.shift_keys(&[Ticks(0), Ticks(100)], Ticks(200)));
        assert!(param.shift_keys(&[Ticks(0), Ticks(100)], Ticks(250)));
        let keys: Vec<(Ticks, f32)> = param.keys.iter().map(|k| (k.time, k.value())).collect();
        assert_eq!(
            keys,
            [
                (Ticks(200), 20.0),
                (Ticks(250), 0.0),
                (Ticks(300), 30.0),
                (Ticks(350), 10.0)
            ]
        );
    }

    #[test]
    fn a_key_moved_past_another_keeps_the_keys_in_order() {
        let mut param = Param::from_value(0.0);
        param.toggle_animation(Ticks(0));
        param.set_at(Ticks(100), 10.0);
        param.set_at(Ticks(200), 20.0);
        assert!(param.move_key(Ticks(0), Ticks(300)));
        let times: Vec<Ticks> = param.keys.iter().map(|key| key.time).collect();
        assert_eq!(times, [Ticks(100), Ticks(200), Ticks(300)]);
        assert_eq!(param.value_at(Ticks(300)), 0.0);
        assert!(!param.move_key(Ticks(100), Ticks(200)));
    }

    #[test]
    fn stopping_animation_keeps_the_value_at_the_playhead() {
        let mut param = Param::from_value(0.0);
        param.toggle_animation(Ticks(0));
        param.set_at(Ticks(100), 10.0);
        param.toggle_animation(Ticks(50));
        assert!(!param.is_animated());
        assert_eq!(param.value(), 5.0);
    }

    #[test]
    fn interpolation_holds_or_eases_each_segment() {
        let mut param = Param::from_value(0.0);
        param.toggle_animation(Ticks(0));
        param.set_at(Ticks(100), 100.0);
        param.set_at(Ticks(200), 0.0);
        assert!(param.set_interpolation(Ticks(0), Interpolation::Hold));
        assert_eq!(param.value_at(Ticks(99)), 0.0);
        assert_eq!(param.value_at(Ticks(100)), 100.0);

        param.set_interpolation(Ticks(100), Interpolation::Ease);
        let quarter = param.value_at(Ticks(125));
        assert!(quarter > 75.0, "{quarter}");
        assert_eq!(param.value_at(Ticks(150)), 62.5);

        // Easing only into the last key leaves the start of the segment linear.
        param.set_interpolation(Ticks(100), Interpolation::EaseIn);
        param.set_interpolation(Ticks(200), Interpolation::EaseIn);
        let start = param.value_at(Ticks(101));
        assert!((start - 99.0).abs() < 0.05, "{start}");
        assert!(param.value_at(Ticks(199)) < 0.05);
    }

    #[test]
    fn auto_bezier_curves_through_a_key_without_overshooting_a_peak() {
        let mut param = Param::from_value(0.0);
        param.toggle_animation(Ticks(0));
        param.set_at(Ticks(100), 50.0);
        param.set_at(Ticks(200), 200.0);
        param.set_at(Ticks(300), 0.0);
        for time in [100, 200] {
            param.set_interpolation(Ticks(time), Interpolation::AutoBezier);
        }
        // Through the rising key at the slope between its neighbours, one per tick,
        // rather than kinking from half a unit a tick to one and a half.
        let slopes =
            [99, 100].map(|time| param.value_at(Ticks(time + 1)) - param.value_at(Ticks(time)));
        assert!(
            slopes.iter().all(|slope| (slope - 1.0).abs() < 0.03),
            "{slopes:?}"
        );
        // Level across the peak, never above it.
        let around = [190, 199, 200, 201, 210].map(|time| param.value_at(Ticks(time)));
        assert!(around.iter().all(|value| *value <= 200.0), "{around:?}");
        assert!(around[1] > 199.9 && around[3] > 199.9, "{around:?}");
    }

    #[test]
    fn bezier_handles_set_each_side_and_continuous_handles_stay_in_line() {
        let mut param = Param::from_value(0.0);
        param.toggle_animation(Ticks(0));
        param.set_at(Ticks(1000), 100.0);
        param.set_at(Ticks(2000), 100.0);
        let key = Ticks(1000);
        let slope = |param: &Param, from: i64| {
            param.value_at(Ticks(from + 1)) - param.value_at(Ticks(from))
        };
        // A Bezier key starts on the auto Bezier path, so nothing moves yet.
        param.set_interpolation(key, Interpolation::AutoBezier);
        let smooth = [500, 1500].map(|time| param.value_at(Ticks(time)));
        assert!(param.set_interpolation(key, Interpolation::Bezier));
        assert_eq!([500, 1500].map(|time| param.value_at(Ticks(time))), smooth);

        // Arriving level and leaving rising, a tenth of a unit a tick.
        assert!(param.set_handle(key, HandleSide::Incoming, 0.0));
        assert!(param.set_handle(key, HandleSide::Outgoing, 10.0));
        let (into, out) = (slope(&param, 999), slope(&param, 1000));
        assert!(
            into.abs() < 0.002 && (out - 0.1).abs() < 0.002,
            "{into} {out}"
        );

        // Continuous joins the sides at their average and moves both together.
        param.set_interpolation(key, Interpolation::ContinuousBezier);
        assert!((slope(&param, 1000) - 0.05).abs() < 0.002);
        param.set_handle(key, HandleSide::Incoming, -5.0);
        let slopes = [999, 1000].map(|from| slope(&param, from));
        assert!(
            slopes.iter().all(|slope| (slope + 0.05).abs() < 0.002),
            "{slopes:?}"
        );

        param.set_interpolation(key, Interpolation::Linear);
        assert_eq!(param.keys[1].handles, None);
        assert!(!param.set_handle(key, HandleSide::Incoming, 0.0));
    }
}
