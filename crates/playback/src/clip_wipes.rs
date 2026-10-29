use render::{BlockDissolve, ClipWipes, LinearWipe, RadialSweep, RadialWipe, VenetianBlinds};
use timeline::{Param, RadialWipeDirection, Ticks, VideoEffect};

/// Sets the Transition effect `effect` in `wipes`, leaving other effects alone.
pub(crate) fn set_clip_wipe(wipes: &mut ClipWipes, effect: &VideoEffect, time: Ticks) {
    let value = |param: &Param| param.value_at(time);
    let unit = |param: &Param| value(param) / 100.0;
    let radians = |param: &Param| value(param).to_radians();
    match effect {
        VideoEffect::LinearWipe {
            completion,
            angle,
            feather,
        } => {
            wipes.linear = Some(LinearWipe {
                completion: unit(completion),
                angle: radians(angle),
                feather: value(feather),
            });
        }
        VideoEffect::RadialWipe {
            completion,
            start_angle,
            center,
            direction,
            feather,
        } => {
            wipes.radial = Some(RadialWipe {
                completion: unit(completion),
                start_angle: radians(start_angle),
                center: center.each_ref().map(unit),
                sweep: match direction {
                    RadialWipeDirection::Clockwise => RadialSweep::Clockwise,
                    RadialWipeDirection::Counterclockwise => RadialSweep::Counterclockwise,
                    RadialWipeDirection::Both => RadialSweep::Both,
                },
                feather: value(feather),
            });
        }
        VideoEffect::VenetianBlinds {
            completion,
            direction,
            width,
            feather,
        } => {
            wipes.blinds = Some(VenetianBlinds {
                completion: unit(completion),
                direction: radians(direction),
                width: value(width),
                feather: value(feather),
            });
        }
        VideoEffect::BlockDissolve {
            completion,
            block_width,
            block_height,
            feather,
        } => {
            wipes.blocks = Some(BlockDissolve {
                completion: unit(completion),
                size: [value(block_width), value(block_height)],
                feather: value(feather),
            });
        }
        _ => {}
    }
}
