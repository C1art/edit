use serde::{Deserialize, Serialize};

use crate::Ticks;

/// How much source time a clip plays per tick of sequence time, as the exact fraction
/// `num / den`: 2/1 plays twice as fast, 1/2 in slow motion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Speed {
    num: u64,
    den: u64,
}

impl Default for Speed {
    fn default() -> Self {
        Self::NORMAL
    }
}

impl Speed {
    pub const NORMAL: Self = Self { num: 1, den: 1 };
    /// 1%, as slow as other editors go.
    pub const MIN: Self = Self { num: 1, den: 100 };
    /// 10,000%.
    pub const MAX: Self = Self { num: 100, den: 1 };

    /// `None` when either part is zero.
    pub fn new(num: u64, den: u64) -> Option<Self> {
        if num == 0 || den == 0 {
            return None;
        }
        let divisor = gcd(num, den);
        Some(Self {
            num: num / divisor,
            den: den / divisor,
        })
    }

    /// The speed in hundredths of a percent, so 10_000 is normal speed.
    pub fn from_basis_points(points: u64) -> Option<Self> {
        Self::new(points, 10_000)
    }

    /// The speed that plays `source` ticks of media in `duration` ticks of sequence.
    pub fn stretch(source: Ticks, duration: Ticks) -> Option<Self> {
        Self::new(
            u64::try_from(source.0).ok()?,
            u64::try_from(duration.0).ok()?,
        )
    }

    pub fn is_normal(self) -> bool {
        self == Self::NORMAL
    }

    pub fn num(self) -> u64 {
        self.num
    }

    pub fn den(self) -> u64 {
        self.den
    }

    /// For display only; the model never stores it.
    pub fn percent(self) -> f64 {
        self.num as f64 / self.den as f64 * 100.0
    }

    /// The source time that plays in `sequence` ticks, rounded down.
    pub fn source_span(self, sequence: Ticks) -> Ticks {
        scale(sequence, self.num, self.den)
    }

    /// The sequence time it takes to play `source` ticks, rounded down.
    pub fn sequence_span(self, source: Ticks) -> Ticks {
        scale(source, self.den, self.num)
    }

    pub fn is_in_range(self) -> bool {
        let at_least = |a: Self, b: Self| {
            u128::from(a.num) * u128::from(b.den) >= u128::from(b.num) * u128::from(a.den)
        };
        at_least(self, Self::MIN) && at_least(Self::MAX, self)
    }
}

/// `ticks * num / den`, rounded toward negative infinity so that spans placed end to
/// end never overlap.
fn scale(ticks: Ticks, num: u64, den: u64) -> Ticks {
    let product = i128::from(ticks.0) * i128::from(num);
    let quotient = product.div_euclid(i128::from(den.max(1)));
    Ticks(i64::try_from(quotient).unwrap_or(if quotient < 0 { i64::MIN } else { i64::MAX }))
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_round_down_and_a_stretch_maps_the_source_exactly_onto_the_duration() {
        let third = Speed::new(2, 6).unwrap();
        assert_eq!((third.num(), third.den()), (1, 3));
        assert_eq!(third.source_span(Ticks(10)), Ticks(3));
        assert_eq!(third.source_span(Ticks(-10)), Ticks(-4));
        assert_eq!(third.sequence_span(Ticks(3)), Ticks(9));

        let stretch = Speed::stretch(Ticks(270_000), Ticks(123_457)).unwrap();
        assert_eq!(stretch.source_span(Ticks(123_457)), Ticks(270_000));
        assert!(stretch.is_in_range());
        assert!(!Speed::new(1, 101).unwrap().is_in_range());
        assert!(!Speed::new(101, 1).unwrap().is_in_range());
    }
}
