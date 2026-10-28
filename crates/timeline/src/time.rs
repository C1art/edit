use std::ops::{Add, Sub};

use serde::{Deserialize, Serialize};

/// Seconds per tick, as the exact fraction `num / den` (e.g. `1001/30000` for 29.97 fps).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Timebase {
    num: u32,
    den: u32,
}

impl Timebase {
    /// The MPEG transport clock. Not exact for 23.976 or 59.94 fps; sequences at those
    /// rates should use their frame duration as the timebase.
    pub const HZ_90K: Self = Self {
        num: 1,
        den: 90_000,
    };

    pub const fn new(num: u32, den: u32) -> Option<Self> {
        if num == 0 || den == 0 {
            None
        } else {
            Some(Self { num, den })
        }
    }

    pub const fn num(self) -> u32 {
        self.num
    }

    pub const fn den(self) -> u32 {
        self.den
    }
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct Ticks(pub i64);

impl Ticks {
    pub const ZERO: Self = Self(0);

    /// Converts to the nearest tick in `to`, rounding halves away from zero and
    /// saturating at the `i64` range.
    pub fn rescale(self, from: Timebase, to: Timebase) -> Self {
        let numerator = i128::from(self.0) * i128::from(from.num) * i128::from(to.den);
        let denominator = i128::from(from.den) * i128::from(to.num);
        let half = denominator / 2;
        let rounded = if numerator >= 0 {
            (numerator + half) / denominator
        } else {
            (numerator - half) / denominator
        };
        let clamped = rounded.clamp(i128::from(i64::MIN), i128::from(i64::MAX));
        Self(i64::try_from(clamped).unwrap_or_default())
    }
}

impl Add for Ticks {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl Sub for Ticks {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tb(num: u32, den: u32) -> Timebase {
        Timebase::new(num, den).unwrap()
    }

    #[test]
    fn rescale_ntsc_frame_to_90khz_is_exact() {
        assert_eq!(Ticks(1).rescale(tb(1001, 30000), tb(1, 90000)), Ticks(3003));
    }

    #[test]
    fn rescale_rounds_halves_away_from_zero() {
        let (fine, coarse) = (tb(1, 90000), tb(1, 30));
        assert_eq!(Ticks(1500).rescale(fine, coarse), Ticks(1));
        assert_eq!(Ticks(-1500).rescale(fine, coarse), Ticks(-1));
        assert_eq!(Ticks(1499).rescale(fine, coarse), Ticks(0));
    }

    #[test]
    fn rescale_saturates_instead_of_overflowing() {
        assert_eq!(
            Ticks(i64::MAX).rescale(tb(1, 1), tb(1, 90000)),
            Ticks(i64::MAX)
        );
    }
}
