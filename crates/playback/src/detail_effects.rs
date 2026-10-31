use render::{Detail, MedianFilter, NoiseHls, UnsharpMask};
use timeline::{Param, Ticks, VideoEffect};

/// Sets Unsharp Mask, Median, Dust & Scratches, Edge Feather or Noise HLS in `detail`,
/// leaving other effects alone.
pub(crate) fn set_detail_effect(detail: &mut Detail, effect: &VideoEffect, time: Ticks) {
    let value = |param: &Param| param.value_at(time);
    let level = |param: &Param| value(param) / 255.0;
    let percent = |param: &Param| value(param) / 100.0;
    match effect {
        VideoEffect::UnsharpMask(unsharp) => {
            detail.unsharp = Some(UnsharpMask {
                amount: percent(&unsharp.amount),
                radius: value(&unsharp.radius),
                threshold: level(&unsharp.threshold),
            });
        }
        VideoEffect::Median(median) | VideoEffect::DustAndScratches(median) => {
            detail.median = Some(MedianFilter {
                radius: value(&median.radius),
                threshold: level(&median.threshold),
            });
        }
        VideoEffect::EdgeFeather(feather) => detail.edge_feather = value(&feather.amount),
        VideoEffect::NoiseHls(noise) => {
            detail.noise = Some(NoiseHls {
                hue: percent(&noise.hue),
                lightness: percent(&noise.lightness),
                saturation: percent(&noise.saturation),
                grain_size: value(&noise.grain_size),
                phase: value(&noise.phase).to_radians(),
            });
        }
        _ => {}
    }
}
