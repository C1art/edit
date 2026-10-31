use render::{AlphaMix, NoiseHls, NoisyAlpha, Overflow, PixelEffects};
use timeline::{AlphaNoise, AlphaOverflow, OriginalAlpha, Param, Ticks, Timebase, VideoEffect};

/// Sets Noise HLS Auto or Noise Alpha in `pixel`, leaving other effects alone.
pub(crate) fn set_noise_effect(
    pixel: &mut PixelEffects,
    effect: &VideoEffect,
    time: Ticks,
    timebase: Timebase,
) {
    let value = |param: &Param| param.value_at(time);
    let percent = |param: &Param| value(param) / 100.0;
    match effect {
        VideoEffect::NoiseHlsAuto(noise) => {
            let seconds = time.0 as f64 * f64::from(timebase.num()) / f64::from(timebase.den());
            let turns = seconds * f64::from(value(&noise.animation_speed).max(0.0));
            pixel.detail.noise = Some(NoiseHls {
                hue: percent(&noise.hue),
                lightness: percent(&noise.lightness),
                saturation: percent(&noise.saturation),
                grain_size: value(&noise.grain_size),
                // Wrapped, so a long clip's phase stays precise in the shader's f32.
                phase: ((turns % 1024.0) * std::f64::consts::TAU) as f32,
            });
        }
        VideoEffect::NoiseAlpha(noise) => {
            let animated = matches!(
                noise.noise,
                AlphaNoise::UniformAnimation | AlphaNoise::SquaredAnimation
            );
            pixel.noise_alpha = Some(NoisyAlpha {
                amount: percent(&noise.amount),
                squared: matches!(
                    noise.noise,
                    AlphaNoise::SquaredRandom | AlphaNoise::SquaredAnimation
                ),
                mix: match noise.original_alpha {
                    OriginalAlpha::Add => AlphaMix::Add,
                    OriginalAlpha::Clamp => AlphaMix::Clamp,
                    OriginalAlpha::Scale => AlphaMix::Scale,
                    OriginalAlpha::UseNoiseAsMask => AlphaMix::Mask,
                },
                overflow: match noise.overflow {
                    AlphaOverflow::Clip => Overflow::Clip,
                    AlphaOverflow::WrapBack => Overflow::WrapBack,
                    AlphaOverflow::WrapAround => Overflow::WrapAround,
                },
                // Kept small: the shader's hash loses precision on large salts.
                seed: (noise.random_seed % 997) as f32 * 0.37,
                phase: animated.then(|| value(&noise.noise_phase) / 360.0),
                cycle: noise.cycle_noise.then(|| value(&noise.cycle)),
            });
        }
        _ => {}
    }
}
