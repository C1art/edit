use render::{AlphaGlow, Bevel, ColorEmboss, Edges, RoughenEdges};
use timeline::{Param, RoughenEdgeType, Ticks, VideoEffect};

use crate::pixel_effects::linear;

/// Pixels across the fractal's largest features at Roughen Edges' 100 percent scale.
const ROUGHEN_FEATURE: f32 = 32.0;

/// Sets Roughen Edges, Bevel Edges, Bevel Alpha, Alpha Glow or Color Emboss in `edges`,
/// leaving other effects alone.
pub(crate) fn set_edge_effect(edges: &mut Edges, effect: &VideoEffect, time: Ticks) {
    let value = |param: &Param| param.value_at(time);
    let percent = |param: &Param| value(param) / 100.0;
    let bevel = |bevel: &timeline::Bevel, thickness: f32| Bevel {
        thickness,
        light_angle: value(&bevel.light_angle).to_radians(),
        light_color: linear(bevel.light_color),
        light_intensity: percent(&bevel.light_intensity),
    };
    match effect {
        VideoEffect::RoughenEdges(rough) => {
            edges.roughen = Some(RoughenEdges {
                border: value(&rough.border),
                sharpness: value(&rough.edge_sharpness),
                fractal_influence: value(&rough.fractal_influence),
                scale: percent(&rough.scale) * ROUGHEN_FEATURE,
                offset: rough.offset.each_ref().map(value),
                complexity: value(&rough.complexity),
                // A full turn moves as far through the fractal as one feature.
                evolution: value(&rough.evolution) / 360.0,
                edge_color: (rough.edge_type == RoughenEdgeType::RoughenColor)
                    .then(|| linear(rough.edge_color)),
            });
        }
        VideoEffect::BevelEdges(edge) => {
            edges.bevel = Some(bevel(edge, percent(&edge.edge_thickness)));
        }
        VideoEffect::BevelAlpha(alpha) => {
            edges.bevel_alpha = Some(bevel(alpha, value(&alpha.edge_thickness)));
        }
        VideoEffect::AlphaGlow(glow) => {
            edges.alpha_glow = Some(AlphaGlow {
                size: value(&glow.glow),
                brightness: value(&glow.brightness) / 255.0,
                start_color: linear(glow.start_color),
                end_color: glow.end_color.map(linear),
                fade_out: glow.fade_out,
            });
        }
        VideoEffect::ColorEmboss(timeline::ColorEmboss {
            direction,
            relief,
            contrast,
            blend,
        }) => {
            edges.color_emboss = Some(ColorEmboss {
                direction: value(direction).to_radians(),
                relief: value(relief),
                contrast: percent(contrast),
                blend: percent(blend),
            });
        }
        _ => {}
    }
}
