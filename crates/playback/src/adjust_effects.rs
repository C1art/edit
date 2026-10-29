use render::{Adjust, Levels, ProcAmp};
use timeline::{Param, Ticks, VideoEffect};

/// Sets Levels or ProcAmp in `adjust`, leaving other effects alone.
pub(crate) fn set_adjust_effect(adjust: &mut Adjust, effect: &VideoEffect, time: Ticks) {
    let value = |param: &Param| param.value_at(time);
    let level = |param: &Param| value(param) / 255.0;
    match effect {
        VideoEffect::Levels {
            input_black,
            input_white,
            output_black,
            output_white,
            gamma,
        } => {
            adjust.levels = Levels {
                input: [level(input_black), level(input_white)],
                output: [level(output_black), level(output_white)],
                gamma: value(gamma),
            };
        }
        VideoEffect::ProcAmp {
            brightness,
            contrast,
            hue,
            saturation,
        } => {
            adjust.proc_amp = ProcAmp {
                brightness: level(brightness),
                contrast: value(contrast) / 100.0,
                hue: value(hue).to_radians(),
                saturation: value(saturation) / 100.0,
            };
        }
        _ => {}
    }
}
