use render::{ChromaKey, PixelEffects};
use timeline::{Rgb, Ticks, VideoEffect};

/// The per-pixel part of an effect stack at source time `time`. The shader runs each
/// kind of effect once, so repeats of a kind combine, the last one's colors winning.
pub(crate) fn pixel_effects(effects: &[VideoEffect], time: Ticks) -> PixelEffects {
    let unit = |param: &timeline::Param| param.value_at(time) / 100.0;
    let mut pixel = PixelEffects::NONE;
    for effect in effects {
        match effect {
            VideoEffect::GaussianBlur { .. } => {}
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
