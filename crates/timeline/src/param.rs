use serde::{Deserialize, Serialize};

use crate::Ticks;

/// A value a parameter passes through at one source time of its clip, so keyframes
/// stay on the same picture when the clip is trimmed, split or moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Keyframe {
    /// In the clip's source time, in the sequence timebase.
    pub time: Ticks,
    pub hundredths: i32,
}

/// An adjustable clip property, held in hundredths of its display unit so the model
/// stays `Eq` and round-trips exactly. With keyframes it is animated and interpolates
/// linearly between them, holding the first and last beyond them.
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
            (Some(before), Some(next)) => {
                let span = (next.time - before.time).0 as f64;
                let progress = (time - before.time).0 as f64 / span;
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
            Err(index) => self.keys.insert(index, Keyframe { time, hundredths }),
        }
    }

    /// Starts animating with a keyframe at `time`, or stops and keeps the value there.
    pub fn toggle_animation(&mut self, time: Ticks) {
        let hundredths = to_hundredths(self.value_at(time));
        if self.is_animated() {
            self.keys.clear();
            self.hundredths = hundredths;
        } else {
            self.keys.push(Keyframe { time, hundredths });
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
            Err(index) => self.keys.insert(
                index,
                Keyframe {
                    time,
                    hundredths: to_hundredths(value),
                },
            ),
        }
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
    fn stopping_animation_keeps_the_value_at_the_playhead() {
        let mut param = Param::from_value(0.0);
        param.toggle_animation(Ticks(0));
        param.set_at(Ticks(100), 10.0);
        param.toggle_animation(Ticks(50));
        assert!(!param.is_animated());
        assert_eq!(param.value(), 5.0);
    }
}
