use render::{FlareLens, LensFlare, Magnify, Optics};
use timeline::{LensType, MagnifyShape, Param, Ticks, VideoEffect};

/// Sets Lens Flare or Magnify in `optics`, leaving other effects alone.
pub(crate) fn set_optics_effect(optics: &mut Optics, effect: &VideoEffect, time: Ticks) {
    let unit = |param: &Param| param.value_at(time) / 100.0;
    match effect {
        VideoEffect::LensFlare {
            center,
            brightness,
            lens,
            blend,
        } => {
            optics.lens_flare = Some(LensFlare {
                center: center.each_ref().map(unit),
                brightness: unit(brightness),
                lens: match lens {
                    LensType::Zoom50To300 => FlareLens::Zoom,
                    LensType::Prime35 => FlareLens::Prime35,
                    LensType::Prime105 => FlareLens::Prime105,
                },
                blend: unit(blend),
            });
        }
        VideoEffect::Magnify {
            shape,
            center,
            magnification,
            size,
            feather,
            opacity,
        } => {
            optics.magnify = Some(Magnify {
                center: center.each_ref().map(unit),
                magnification: unit(magnification),
                size: size.value_at(time),
                feather: feather.value_at(time),
                opacity: unit(opacity),
                square: *shape == MagnifyShape::Square,
            });
        }
        _ => {}
    }
}
