use render::{ChromaKey, ColorKey, LeaveColor, Mirror, PixelEffects, Ramp};
use timeline::{ColorMatch, RampShape, Rgb, Ticks, Timebase, VideoEffect};

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
            VideoEffect::GaussianBlur { .. } | VideoEffect::TrackMatteKey { .. } => {}
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
                pixel.reshape.corner_pin =
                    Some(corners.each_ref().map(|corner| corner.each_ref().map(unit)));
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
            VideoEffect::PosterizeTime { .. } | VideoEffect::Timecode { .. } => {}
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
            } => {
                pixel.key = Some(ChromaKey {
                    color: linear(*color),
                    tolerance: unit(tolerance).clamp(0.0, 1.0),
                    softness: unit(softness).clamp(0.0, 1.0),
                });
            }
        }
    }
    pixel
}

fn encoded(color: Rgb) -> [f32; 3] {
    [color.r, color.g, color.b].map(|channel| f32::from(channel) / 255.0)
}

fn linear(color: Rgb) -> [f32; 3] {
    [color.r, color.g, color.b].map(|channel| {
        let encoded = f32::from(channel) / 255.0;
        if encoded <= 0.04045 {
            encoded / 12.92
        } else {
            ((encoded + 0.055) / 1.055).powf(2.4)
        }
    })
}
