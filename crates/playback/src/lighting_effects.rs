use render::{Lamp, LampKind, Lighting, PixelEffects, PointShadow};
use timeline::{Light, LightKind, LightingEffects, RadialShadow, ShadowRender, Ticks};

use crate::pixel_effects::linear;

/// Lighting Effects at `time`, from Premiere's ranges: percents of the frame for
/// points and radii, degrees for the angle and −100 to 100 for the focus.
pub(crate) fn lighting(effect: &LightingEffects, time: Ticks) -> Lighting {
    let percent = |param: &timeline::Param| param.value_at(time) / 100.0;
    Lighting {
        lights: std::array::from_fn(|index| {
            let light = effect.lights.get(index)?;
            lamp(light, time)
        }),
        ambience: linear(effect.ambience_color),
        ambience_intensity: percent(&effect.ambience_intensity).max(0.0),
        gloss: percent(&effect.surface_gloss),
        material: percent(&effect.surface_material),
        exposure: (effect.exposure.value_at(time) / 50.0).exp2(),
        // A full height of 100 tilts the surface ten times the brightness step.
        bump: percent(&effect.bump_height) * 10.0,
        white_is_high: effect.white_is_high,
    }
}

/// Radial Shadow at `time`, which takes over the picture's drop shadow.
pub(crate) fn set_radial_shadow(pixel: &mut PixelEffects, shadow: &RadialShadow, time: Ticks) {
    let percent = |param: &timeline::Param| param.value_at(time) / 100.0;
    pixel.style.shadow_offset = [0.0; 2];
    pixel.style.shadow_softness = shadow.softness.value_at(time).max(0.0);
    pixel.style.shadow_opacity = percent(&shadow.opacity).clamp(0.0, 1.0);
    pixel.style.shadow_color = linear(shadow.shadow_color);
    pixel.point_shadow = Some(PointShadow {
        light: shadow.light_source.each_ref().map(percent),
        scale: 1.0 + percent(&shadow.projection_distance).max(0.0),
        shadow_only: shadow.shadow_only,
        glass_influence: (shadow.render == ShadowRender::GlassEdge)
            .then(|| percent(&shadow.color_influence)),
    });
}

fn lamp(light: &Light, time: Ticks) -> Option<Lamp> {
    let kind = match light.kind {
        LightKind::None => return None,
        LightKind::Directional => LampKind::Directional,
        LightKind::Omni => LampKind::Omni,
        LightKind::Spotlight => LampKind::Spotlight,
    };
    let percent = |param: &timeline::Param| param.value_at(time) / 100.0;
    Some(Lamp {
        kind,
        color: linear(light.color),
        center: light.center.each_ref().map(percent),
        major_radius: percent(&light.major_radius),
        minor_radius: percent(&light.minor_radius),
        angle: light.angle.value_at(time).to_radians(),
        // Premiere's default intensity of 20 lights the picture as it is.
        intensity: light.intensity.value_at(time) / 20.0,
        focus: (light.focus.value_at(time) + 100.0) / 200.0,
    })
}
