use serde::{Deserialize, Serialize};

use crate::Ticks;

/// A value a parameter passes through at one source time of its clip, so keyframes
/// stay on the same picture when the clip is trimmed, split or moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Keyframe {
    /// In the clip's source time, in the sequence timebase.
    pub time: Ticks,
    pub hundredths: i32,
    #[serde(default, skip_serializing_if = "Interpolation::is_linear")]
    pub interpolation: Interpolation,
}

impl Keyframe {
    pub const fn new(time: Ticks, hundredths: i32) -> Self {
        Self {
            time,
            hundredths,
            interpolation: Interpolation::Linear,
        }
    }
}

/// How the value moves through a keyframe. Easing slows the change arriving at the
/// keyframe (in), leaving it (out) or both; a hold keeps the value until the next one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Interpolation {
    #[default]
    Linear,
    Hold,
    EaseIn,
    EaseOut,
    Ease,
}

impl Interpolation {
    pub const ALL: [Self; 5] = [
        Self::Linear,
        Self::Hold,
        Self::EaseIn,
        Self::EaseOut,
        Self::Ease,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Linear => "Linear",
            Self::Hold => "Hold",
            Self::EaseIn => "Ease In",
            Self::EaseOut => "Ease Out",
            Self::Ease => "Ease In and Out",
        }
    }

    fn is_linear(&self) -> bool {
        *self == Self::Linear
    }

    fn eases_in(self) -> bool {
        matches!(self, Self::EaseIn | Self::Ease)
    }

    fn eases_out(self) -> bool {
        matches!(self, Self::EaseOut | Self::Ease)
    }
}

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
                let progress = eased(
                    (time - before.time).0 as f64 / span,
                    before.interpolation.eases_out(),
                    next.interpolation.eases_in(),
                );
                let (from, to) = (f64::from(before.hundredths), f64::from(next.hundredths));
                from + (to - from) * progress
            }
        };
        (hundredths / 100.0) as f32
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

    pub fn interpolation_at(&self, time: Ticks) -> Option<Interpolation> {
        self.keys
            .iter()
            .find(|key| key.time == time)
            .map(|key| key.interpolation)
    }

    /// Returns whether there was a keyframe at `time` to change.
    pub fn set_interpolation(&mut self, time: Ticks, interpolation: Interpolation) -> bool {
        let key = self.keys.iter_mut().find(|key| key.time == time);
        key.map(|key| key.interpolation = interpolation).is_some()
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

/// `progress` through a segment reshaped by a cubic Hermite curve whose slope is zero
/// at an eased end and one at a linear end, so a segment with neither eased stays linear.
fn eased(progress: f64, out_of_start: bool, into_end: bool) -> f64 {
    let slope = |eased: bool| if eased { 0.0 } else { 1.0 };
    let p = progress;
    let (p2, p3) = (p * p, p * p * p);
    (p3 - 2.0 * p2 + p) * slope(out_of_start) + (3.0 * p2 - 2.0 * p3) + (p3 - p2) * slope(into_end)
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
}
