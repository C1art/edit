use serde::{Deserialize, Serialize};

use crate::curves::smooth_through;
use crate::{CurvePoint, CURVE_RANGE};

/// A line drawn over hue, lightness or saturation whose height changes the colors there:
/// flat through the middle until points bend it. Hue curves wrap around, so red at
/// both ends of the line is one color.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct HueCurve {
    /// Sorted by input; outputs at [`HUE_CURVE_NEUTRAL`] leave colors alone.
    pub points: Vec<CurvePoint>,
}

/// The output height that changes nothing.
pub const HUE_CURVE_NEUTRAL: u16 = CURVE_RANGE / 2;

/// Lumetri's Hue Saturation Curves, each read off at a color's own hue, lightness or
/// saturation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct HueCurves {
    /// Up doubles the saturation of the hues under it, down greys them.
    pub hue_vs_sat: HueCurve,
    /// Up or down turns the hues under it by up to half a turn.
    pub hue_vs_hue: HueCurve,
    /// Up brightens the hues under it, down darkens them.
    pub hue_vs_luma: HueCurve,
    /// The saturation of each lightness, as for greying blown highlights.
    pub luma_vs_sat: HueCurve,
    /// The saturation of each saturation, as for lifting muted colors alone.
    pub sat_vs_sat: HueCurve,
}

impl HueCurve {
    pub fn is_neutral(&self) -> bool {
        self.points
            .iter()
            .all(|point| point.output == HUE_CURVE_NEUTRAL)
    }

    /// How far above neutral the curve runs at `input`, from -0.5 to 0.5, with inputs
    /// running 0 to 1. A curve that `wraps` joins its last point to its first across
    /// the ends.
    pub fn offset(&self, input: f32, wraps: bool) -> f32 {
        let unit = |value: u16| value as f32 / CURVE_RANGE as f32;
        let mut points: Vec<[f32; 2]> = self
            .points
            .iter()
            .map(|point| [unit(point.input), unit(point.output) - 0.5])
            .collect();
        if wraps && !points.is_empty() {
            // Copies a turn away on each side carry the curve smoothly across the ends.
            let before = points.iter().map(|&[x, y]| [x - 1.0, y]);
            let after = points.iter().map(|&[x, y]| [x + 1.0, y]);
            points = before.chain(points.clone()).chain(after).collect();
        }
        smooth_through(&points, input).map_or(0.0, |offset| offset.clamp(-0.5, 0.5))
    }

    /// Adds a point, or moves the one already at `input`, keeping the points sorted.
    pub fn set(&mut self, point: CurvePoint) {
        match self
            .points
            .binary_search_by_key(&point.input, |existing| existing.input)
        {
            Ok(index) => self.points[index] = point,
            Err(index) => self.points.insert(index, point),
        }
    }
}

impl HueCurves {
    pub fn is_neutral(&self) -> bool {
        self.all().iter().all(|curve| curve.is_neutral())
    }

    pub fn all(&self) -> [&HueCurve; 5] {
        [
            &self.hue_vs_sat,
            &self.hue_vs_hue,
            &self.hue_vs_luma,
            &self.luma_vs_sat,
            &self.sat_vs_sat,
        ]
    }

    /// Two rows of `size` entries over inputs from 0 to 1: by hue, the saturation
    /// factor, the turn of hue and the lightness offset; then the saturation factors by
    /// lightness and by saturation, with an unused third.
    pub fn table(&self, size: usize) -> Vec<[f32; 3]> {
        let last = size.saturating_sub(1).max(1) as f32;
        let factor = |offset: f32| 2.0 * (offset + 0.5);
        let inputs = || (0..size).map(|index| index as f32 / last);
        let by_hue = inputs().map(|hue| {
            [
                factor(self.hue_vs_sat.offset(hue, true)),
                self.hue_vs_hue.offset(hue, true),
                self.hue_vs_luma.offset(hue, true),
            ]
        });
        let by_tone = inputs().map(|level| {
            [
                factor(self.luma_vs_sat.offset(level, false)),
                factor(self.sat_vs_sat.offset(level, false)),
                0.0,
            ]
        });
        by_hue.chain(by_tone).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn curve(points: &[(u16, u16)]) -> HueCurve {
        HueCurve {
            points: points
                .iter()
                .map(|&(input, output)| CurvePoint { input, output })
                .collect(),
        }
    }

    #[test]
    fn a_hue_curve_wraps_across_red() {
        // Raised near the top of the hue line, neutral further round.
        let raised = curve(&[(300, 500), (950, 900)]);
        let near_end = raised.offset(0.95, true);
        assert!((near_end - 0.4).abs() < 1e-6);
        // Hue 0 is hue 1, so both ends agree and the curve bends back down through them.
        let (start, end) = (raised.offset(0.0, true), raised.offset(1.0, true));
        assert!((start - end).abs() < 1e-6);
        assert!(start > 0.0 && start < near_end);
        // Without wrapping, the line holds the last point's height to the end.
        assert!((raised.offset(1.0, false) - 0.4).abs() < 1e-6);
        assert_eq!(raised.offset(0.0, false), 0.0);
    }

    #[test]
    fn the_table_scales_saturation_around_one_and_turns_hue_around_zero() {
        let curves = HueCurves {
            hue_vs_sat: curve(&[(0, 0)]),
            hue_vs_hue: curve(&[(0, 750)]),
            sat_vs_sat: curve(&[(0, 1_000)]),
            ..HueCurves::default()
        };
        let table = curves.table(3);
        assert_eq!(table.len(), 6);
        assert_eq!(table[1], [0.0, 0.25, 0.0]);
        assert_eq!(table[4], [1.0, 2.0, 0.0]);
        let neutral = HueCurves::default().table(2);
        assert_eq!(
            neutral,
            [
                [1.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [1.0, 1.0, 0.0]
            ]
        );
    }
}
