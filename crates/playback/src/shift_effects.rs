use render::{ColorShift, ShiftColor, ShiftMatch, ToneBalance};
use timeline::{ChangeColorView, MatchColors, Param, Ticks, VideoEffect};

use crate::pixel_effects::encoded;

/// Sets Change Color or Color Balance in `shift`, leaving other effects alone.
pub(crate) fn set_shift_effect(shift: &mut ColorShift, effect: &VideoEffect, time: Ticks) {
    let percent = |param: &Param| param.value_at(time) / 100.0;
    match effect {
        VideoEffect::ChangeColor(change) => {
            shift.change_color = Some(ShiftColor {
                target: encoded(change.color_to_change),
                match_by: match change.match_colors {
                    MatchColors::Rgb => ShiftMatch::Rgb,
                    MatchColors::Hue => ShiftMatch::Hue,
                    MatchColors::Chroma => ShiftMatch::Chroma,
                },
                hue: change.hue_transform.value_at(time) / 360.0,
                lightness: percent(&change.lightness_transform),
                saturation: percent(&change.saturation_transform),
                tolerance: percent(&change.matching_tolerance),
                softness: percent(&change.matching_softness),
                invert: change.invert_mask,
                show_mask: change.view == ChangeColorView::ColorCorrectionMask,
            });
        }
        VideoEffect::ColorBalance(balance) => {
            shift.balance = Some(ToneBalance {
                shadows: balance.shadows.each_ref().map(percent),
                midtones: balance.midtones.each_ref().map(percent),
                highlights: balance.highlights.each_ref().map(percent),
                preserve_luminosity: balance.preserve_luminosity,
            });
        }
        _ => {}
    }
}
