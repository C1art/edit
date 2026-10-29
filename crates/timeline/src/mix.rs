use serde::{Deserialize, Serialize};

/// A level change in hundredths of a decibel. Integer so clips and tracks stay `Eq` and
/// project files round-trip exactly.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct Gain(pub i32);

impl Gain {
    pub const UNITY: Self = Self(0);
    /// At or below this the signal is treated as silent, like a fader at the bottom.
    pub const SILENT: Self = Self(-96_00);
    pub const MAX: Self = Self(15_00);

    pub fn from_db(db: f32) -> Self {
        let hundredths = (db * 100.0)
            .round()
            .clamp(Self::SILENT.0 as f32, Self::MAX.0 as f32);
        Self(hundredths as i32)
    }

    pub fn db(self) -> f32 {
        self.0 as f32 / 100.0
    }

    pub fn amplitude(self) -> f32 {
        if self <= Self::SILENT {
            0.0
        } else {
            10_f32.powf(self.db() / 20.0)
        }
    }
}

/// Stereo balance from -100 (left only) through 0 (centre) to 100 (right only).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Pan(i16);

impl Pan {
    pub const CENTER: Self = Self(0);

    pub fn new(value: i16) -> Self {
        Self(value.clamp(-100, 100))
    }

    pub fn value(self) -> i16 {
        self.0
    }

    /// Left and right multipliers. A balance control rather than a panner: sources are
    /// stereo, so the centre leaves both sides untouched and turning attenuates one side.
    pub fn balance(self) -> [f32; 2] {
        balance(f32::from(self.0))
    }
}

/// Left and right multipliers for a balance from -100 to 100, as [`Pan::balance`].
pub fn balance(value: f32) -> [f32; 2] {
    let position = value.clamp(-100.0, 100.0) / 100.0;
    [(1.0 - position).min(1.0), (1.0 + position).min(1.0)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gain_converts_decibels_and_floors_to_silence() {
        assert!((Gain::from_db(-6.0).amplitude() - 0.501).abs() < 0.001);
        assert_eq!(Gain::from_db(-200.0), Gain::SILENT);
        assert_eq!(Gain::SILENT.amplitude(), 0.0);
        assert_eq!(Pan::new(-50).balance(), [1.0, 0.5]);
    }
}
