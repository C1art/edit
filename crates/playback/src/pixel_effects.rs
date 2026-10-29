use render::{
    ChangeToColor, ChannelMixer, ChromaKey, ColorKey, LeaveColor, LumaKey, Mirror, PixelEffects,
    PlaneTransform, Ramp, Warp,
};
use timeline::{
    ColorChange, ColorMatch, RampShape, Rgb, Ticks, Timebase, VideoEffect, ULTRA_KEY_REACH,
};

use crate::adjust_effects::set_adjust_effect;
use crate::clip_wipes::set_clip_wipe;
use crate::detail_effects::set_detail_effect;
use crate::edge_effects::set_edge_effect;
use crate::generate_effects::set_generate_effect;
use crate::image_effects::set_image_effect;
use crate::lighting_effects::{lighting, set_radial_shadow};
use crate::noise_effects::set_noise_effect;
use crate::optics_effects::set_optics_effect;
use crate::shift_effects::set_shift_effect;
use crate::tone_effects::set_tone_effect;

/// The per-pixel part of an effect stack at source time `time`. The shader runs each
/// kind of effect once, so repeats of a kind combine, the last one's colors winning.
pub(crate) fn pixel_effects(
    effects: &[VideoEffect],
    time: Ticks,
    timebase: Timebase,
) -> PixelEffects {
    let unit = |param: &timeline::Param| param.value_at(time) / 100.0;
    let mut pixel = PixelEffects::NONE;
    for effect in effects {
        match effect {
            // Blurs fold into the layer's blur, and Echo into the frame decoded.
            VideoEffect::GaussianBlur { .. }
            | VideoEffect::ChannelBlur(_)
            | VideoEffect::CameraBlur { .. }
            | VideoEffect::Echo(_)
            | VideoEffect::GarbageMatte(_)
            | VideoEffect::AutoReframe(_)
            | VideoEffect::TrackMatteKey { .. }
            | VideoEffect::SetMatte { .. } => {}
            VideoEffect::BrightnessContrast {
                brightness,
                contrast,
            } => {
                pixel.style.brightness = unit(brightness).clamp(-1.0, 1.0);
                pixel.style.contrast = unit(contrast).clamp(-1.0, 1.0);
            }
            VideoEffect::GammaCorrection { gamma } => {
                pixel.style.gamma = gamma.value_at(time).max(0.01);
            }
            VideoEffect::Posterize { levels } => {
                pixel.style.posterize = levels.value_at(time).round().clamp(2.0, 64.0);
            }
            VideoEffect::Noise { amount } => {
                pixel.style.noise = unit(amount).clamp(0.0, 1.0);
                // Any number that changes every frame will do; wrapping keeps it exact.
                pixel.style.noise_seed = time.0.rem_euclid(4093) as f32;
            }
            VideoEffect::FindEdges { invert, blend } => {
                pixel.style.edges = 1.0 - unit(blend).clamp(0.0, 1.0);
                pixel.style.edges_inverted = *invert;
            }
            VideoEffect::Mosaic {
                horizontal,
                vertical,
            } => {
                pixel.style.mosaic =
                    [horizontal, vertical].map(|blocks| blocks.value_at(time).round().max(1.0));
            }
            VideoEffect::Offset { shift, blend } => {
                pixel.style.offset = shift.each_ref().map(|to| unit(to) - 0.5);
                pixel.style.offset_mix = 1.0 - unit(blend).clamp(0.0, 1.0);
            }
            VideoEffect::Twirl { angle, radius } => {
                pixel.style.twirl = angle.value_at(time).to_radians();
                pixel.style.twirl_radius = unit(radius).max(0.0);
            }
            VideoEffect::LensDistortion { curvature } => {
                pixel.style.lens = unit(curvature).clamp(-1.0, 1.0);
            }
            VideoEffect::DirectionalBlur { direction, length } => {
                pixel.style.blur_direction = direction.value_at(time).to_radians();
                pixel.style.blur_length = length.value_at(time).max(0.0);
            }
            VideoEffect::DropShadow {
                color,
                opacity,
                direction,
                distance,
                softness,
            } => {
                let toward = direction.value_at(time).to_radians();
                let distance = distance.value_at(time);
                pixel.style.shadow_offset = [toward.sin(), -toward.cos()].map(|d| d * distance);
                pixel.style.shadow_softness = softness.value_at(time).max(0.0);
                pixel.style.shadow_opacity = unit(opacity).clamp(0.0, 1.0);
                pixel.style.shadow_color = linear(*color);
            }
            VideoEffect::WaveWarp {
                height,
                width,
                direction,
                speed,
            } => {
                pixel.style.wave_height = height.value_at(time);
                pixel.style.wave_width = width.value_at(time).max(1.0);
                pixel.style.wave_direction = direction.value_at(time).to_radians();
                // Kept within a wave of zero, so long clips don't lose precision.
                let seconds = time.0 as f64 * f64::from(timebase.num()) / f64::from(timebase.den());
                let waves = seconds * f64::from(speed.value_at(time));
                pixel.style.wave_phase = waves.rem_euclid(1.0) as f32;
            }
            VideoEffect::WarpStabilizer(stabilizer) => {
                if let Some(correction) = stabilizer.correction_at(time) {
                    let [x, y] = correction.shift;
                    pixel.style.stabilize =
                        [x, y, correction.angle, correction.scale].map(|value| value as f32);
                }
            }
            VideoEffect::ColorKey {
                color,
                tolerance,
                softness,
            } => {
                pixel.recolor.color_key = Some(ColorKey {
                    color: encoded(*color),
                    tolerance: unit(tolerance).clamp(0.0, 1.0),
                    softness: unit(softness).clamp(0.0, 1.0),
                });
            }
            VideoEffect::LeaveColor {
                color,
                amount,
                tolerance,
                softness,
                matching,
            } => {
                pixel.recolor.leave_color = Some(LeaveColor {
                    color: encoded(*color),
                    amount: unit(amount).clamp(0.0, 1.0),
                    tolerance: unit(tolerance).clamp(0.0, 1.0),
                    softness: unit(softness).clamp(0.0, 1.0),
                    by_hue: *matching == ColorMatch::Hue,
                    reverse: false,
                });
            }
            VideoEffect::GradientRamp {
                start,
                start_color,
                end,
                end_color,
                shape,
                blend,
            } => {
                pixel.recolor.ramp = Some(Ramp {
                    start: start.each_ref().map(unit),
                    end: end.each_ref().map(unit),
                    start_color: encoded(*start_color),
                    end_color: encoded(*end_color),
                    radial: *shape == RampShape::Radial,
                    blend: unit(blend).clamp(0.0, 1.0),
                });
            }
            VideoEffect::Mirror { center, angle } => {
                pixel.reshape.mirror = Some(Mirror {
                    center: center.each_ref().map(unit),
                    angle: angle.value_at(time).to_radians(),
                });
            }
            VideoEffect::CornerPin { corners } => {
                let corners = corners.each_ref().map(|corner| corner.each_ref().map(unit));
                pixel.reshape.push(Warp::CornerPin(corners));
            }
            VideoEffect::Transform {
                anchor,
                position,
                scale,
                skew,
                skew_axis,
                rotation,
                ..
            } => pixel.reshape.push(Warp::Transform(PlaneTransform {
                anchor: anchor.each_ref().map(unit),
                position: position.each_ref().map(unit),
                scale: scale.each_ref().map(unit),
                skew_degrees: skew.value_at(time),
                skew_axis_degrees: skew_axis.value_at(time),
                rotation_degrees: rotation.value_at(time),
            })),
            VideoEffect::Basic3D {
                swivel,
                tilt,
                distance,
            } => pixel.reshape.push(Warp::Turn {
                swivel: swivel.value_at(time).to_radians(),
                tilt: tilt.value_at(time).to_radians(),
                distance: unit(distance),
                pivot: 0.0,
            }),
            VideoEffect::Spherize { radius, center } => {
                pixel.style.sphere_radius = radius.value_at(time).max(0.0);
                pixel.style.sphere_center = center.each_ref().map(unit);
            }
            VideoEffect::Emboss {
                direction,
                relief,
                contrast,
                blend,
            } => {
                pixel.style.emboss_direction = direction.value_at(time).to_radians();
                pixel.style.emboss_relief = relief.value_at(time).max(0.0);
                pixel.style.emboss_contrast = unit(contrast).max(0.0);
                pixel.style.emboss_mix = 1.0 - unit(blend).clamp(0.0, 1.0);
            }
            VideoEffect::TurbulentDisplace {
                amount,
                size,
                offset,
                complexity,
                evolution,
            } => {
                let size = size.value_at(time).max(1.0);
                pixel.style.turbulence_amount = unit(amount) * size;
                pixel.style.turbulence_size = size;
                pixel.style.turbulence_center = offset.each_ref().map(unit);
                pixel.style.turbulence_complexity = complexity.value_at(time).clamp(1.0, 10.0);
                pixel.style.turbulence_evolution = evolution.value_at(time) / 360.0;
            }
            VideoEffect::ChannelMixer {
                red,
                green,
                blue,
                monochrome,
            } => {
                let row = |row: &[timeline::Param; 4]| row.each_ref().map(unit);
                let red = row(red);
                let rows = if *monochrome {
                    [red; 3]
                } else {
                    [red, row(green), row(blue)]
                };
                pixel.recolor.channel_mixer = Some(ChannelMixer { rows });
            }
            VideoEffect::ChangeToColor {
                from,
                to,
                change,
                tolerance,
                softness,
            } => {
                pixel.recolor.change_to_color = Some(ChangeToColor {
                    from: encoded(*from),
                    to: encoded(*to),
                    change: match change {
                        ColorChange::Hue => render::ColorChange::Hue,
                        ColorChange::HueLightness => render::ColorChange::HueLightness,
                        ColorChange::HueLightnessSaturation => {
                            render::ColorChange::HueLightnessSaturation
                        }
                        ColorChange::Colorize => render::ColorChange::Colorize,
                    },
                    tolerance: tolerance.each_ref().map(|t| unit(t).clamp(0.0, 1.0)),
                    softness: unit(softness).max(0.0),
                });
            }
            VideoEffect::StrobeLight {
                color,
                blend,
                duration,
                period,
            } => {
                let seconds = time.0 as f64 * f64::from(timebase.num()) / f64::from(timebase.den());
                let period = f64::from(period.value_at(time)).max(0.01);
                let flashing = seconds.rem_euclid(period) < f64::from(duration.value_at(time));
                if flashing {
                    // A ramp of one color is a flat fill over the picture.
                    pixel.recolor.ramp = Some(Ramp {
                        start: [0.0, 0.0],
                        end: [0.0, 1.0],
                        start_color: encoded(*color),
                        end_color: encoded(*color),
                        radial: false,
                        blend: 1.0 - unit(blend).clamp(0.0, 1.0),
                    });
                }
            }
            VideoEffect::PosterizeTime { .. }
            | VideoEffect::Timecode { .. }
            | VideoEffect::ClipName { .. }
            | VideoEffect::SimpleText(_)
            | VideoEffect::Lightning(_) => {}
            VideoEffect::HorizontalFlip => pixel.flip[0] = !pixel.flip[0],
            VideoEffect::VerticalFlip => pixel.flip[1] = !pixel.flip[1],
            VideoEffect::BlackAndWhite => {
                pixel.tint = 1.0;
                pixel.tint_black = linear(Rgb::BLACK);
                pixel.tint_white = linear(Rgb::WHITE);
            }
            VideoEffect::Tint {
                black,
                white,
                amount,
            } => {
                pixel.tint = unit(amount).clamp(0.0, 1.0);
                pixel.tint_black = linear(*black);
                pixel.tint_white = linear(*white);
            }
            VideoEffect::Invert { amount } => {
                pixel.invert = (pixel.invert + unit(amount)).clamp(0.0, 1.0);
            }
            VideoEffect::Sharpen { amount } => pixel.sharpen += unit(amount).max(0.0),
            VideoEffect::Vignette {
                amount,
                midpoint,
                feather,
            } => {
                pixel.vignette.amount = unit(amount).clamp(-1.0, 1.0);
                pixel.vignette.midpoint = unit(midpoint).clamp(0.0, 1.0);
                pixel.vignette.feather = unit(feather).clamp(0.0, 1.0);
            }
            VideoEffect::UltraKey {
                color,
                tolerance,
                softness,
                output,
                pedestal,
                contrast,
                choke,
                soften,
                spill,
            } => {
                let share = |param| unit(param).clamp(0.0, 1.0);
                pixel.key = Some(ChromaKey {
                    color: linear(*color),
                    tolerance: share(tolerance),
                    softness: share(softness),
                    pedestal: share(pedestal),
                    contrast: share(contrast),
                    choke: share(choke) * ULTRA_KEY_REACH,
                    soften: share(soften) * ULTRA_KEY_REACH,
                    spill: share(spill),
                    output: *output,
                });
            }
            VideoEffect::LumaKey { threshold, cutoff } => {
                pixel.adjust.luma_key = Some(LumaKey {
                    threshold: unit(threshold).clamp(0.0, 1.0),
                    cutoff: unit(cutoff).clamp(0.0, 1.0),
                });
            }
            VideoEffect::AlphaAdjust {
                opacity,
                ignore_alpha,
                invert_alpha,
                mask_only,
            } => {
                let alpha = &mut pixel.adjust.alpha;
                alpha.opacity *= unit(opacity).clamp(0.0, 1.0);
                alpha.ignore |= *ignore_alpha;
                alpha.invert ^= *invert_alpha;
                alpha.mask_only |= *mask_only;
            }
            VideoEffect::Solarize { threshold } => {
                pixel.adjust.solarize = Some(unit(threshold).clamp(0.0, 1.0));
            }
            VideoEffect::Threshold { level } => {
                pixel.adjust.threshold = Some((level.value_at(time) / 255.0).clamp(0.0, 1.0));
            }
            VideoEffect::Replicate { count } => {
                pixel.adjust.replicate *= count.value_at(time).round().clamp(1.0, 16.0);
            }
            VideoEffect::ColorBalanceRgb { gains } => {
                for (balance, gain) in pixel.adjust.balance.iter_mut().zip(gains) {
                    *balance *= unit(gain).max(0.0);
                }
            }
            VideoEffect::Checkerboard(_)
            | VideoEffect::Grid(_)
            | VideoEffect::Circle(_)
            | VideoEffect::FourColorGradient(_)
            | VideoEffect::Ellipse(_)
            | VideoEffect::CellPattern(_) => set_generate_effect(&mut pixel, effect, time),
            VideoEffect::LightingEffects(effect) => pixel.lighting = Some(lighting(effect, time)),
            VideoEffect::RadialShadow(shadow) => set_radial_shadow(&mut pixel, shadow, time),
            VideoEffect::ColorPass(_)
            | VideoEffect::ReduceInterlaceFlicker(_)
            | VideoEffect::CineonConverter(_) => set_image_effect(&mut pixel, effect, time),
            VideoEffect::NoiseHlsAuto(_) | VideoEffect::NoiseAlpha(_) => {
                set_noise_effect(&mut pixel, effect, time, timebase);
            }
            VideoEffect::LensFlare { .. } | VideoEffect::Magnify { .. } => {
                set_optics_effect(&mut pixel.optics, effect, time);
            }
            VideoEffect::LinearWipe { .. }
            | VideoEffect::RadialWipe { .. }
            | VideoEffect::VenetianBlinds { .. }
            | VideoEffect::BlockDissolve { .. } => {
                set_clip_wipe(&mut pixel.clip_wipes, effect, time);
            }
            VideoEffect::Levels { .. } | VideoEffect::ProcAmp { .. } => {
                set_adjust_effect(&mut pixel.adjust, effect, time);
            }
            VideoEffect::UnsharpMask(_)
            | VideoEffect::Median(_)
            | VideoEffect::DustAndScratches(_)
            | VideoEffect::EdgeFeather(_)
            | VideoEffect::NoiseHls(_) => set_detail_effect(&mut pixel.detail, effect, time),
            VideoEffect::RoughenEdges(_)
            | VideoEffect::BevelEdges(_)
            | VideoEffect::BevelAlpha(_)
            | VideoEffect::AlphaGlow(_)
            | VideoEffect::ColorEmboss { .. } => set_edge_effect(&mut pixel.edges, effect, time),
            VideoEffect::ConvolutionKernel(_)
            | VideoEffect::Extract(_)
            | VideoEffect::ShadowHighlight(_)
            | VideoEffect::VideoLimiter(_)
            | VideoEffect::AscCdl(_)
            | VideoEffect::ColorReplace(_) => set_tone_effect(&mut pixel.tone, effect, time),
            VideoEffect::ChangeColor(_) | VideoEffect::ColorBalance(_) => {
                set_shift_effect(&mut pixel.shift, effect, time)
            }
        }
    }
    pixel
}

pub(crate) fn encoded(color: Rgb) -> [f32; 3] {
    [color.r, color.g, color.b].map(|channel| f32::from(channel) / 255.0)
}

pub(crate) fn linear(color: Rgb) -> [f32; 3] {
    [color.r, color.g, color.b].map(|channel| {
        let encoded = f32::from(channel) / 255.0;
        if encoded <= 0.04045 {
            encoded / 12.92
        } else {
            ((encoded + 0.055) / 1.055).powf(2.4)
        }
    })
}

/// How much the stack's Transform effects let through, each fading the layer further.
pub(crate) fn transform_opacity(effects: &[VideoEffect], time: Ticks) -> f32 {
    effects
        .iter()
        .filter_map(|effect| match effect {
            VideoEffect::Transform { opacity, .. } => {
                Some((opacity.value_at(time) / 100.0).clamp(0.0, 1.0))
            }
            _ => None,
        })
        .product()
}
