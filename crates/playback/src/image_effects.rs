use render::{
    Brush, BrushSurface, Cineon, CineonKind, Conform, Eyedropper, LeaveColor, PixelEffects,
};
use timeline::{CineonConversion, PaintSurface, Param, Ticks, VideoEffect};

use crate::pixel_effects::encoded;

/// Sets Color Pass, Reduce Interlace Flicker, Cineon Converter, Brush Strokes,
/// Eyedropper Fill or SDR Conform in `pixel`, leaving other effects alone.
pub(crate) fn set_image_effect(pixel: &mut PixelEffects, effect: &VideoEffect, time: Ticks) {
    let value = |param: &Param| param.value_at(time);
    let percent = |param: &Param| value(param) / 100.0;
    match effect {
        VideoEffect::ColorPass(pass) => {
            pixel.recolor.leave_color = Some(LeaveColor {
                color: encoded(pass.color),
                amount: 1.0,
                tolerance: percent(&pass.similarity).clamp(0.0, 1.0),
                softness: 0.0,
                by_hue: false,
                reverse: pass.reverse,
            });
        }
        VideoEffect::ReduceInterlaceFlicker(flicker) => {
            // A vertical blur, which evens out detail that lives on only one field.
            pixel.style.blur_direction = 0.0;
            pixel.style.blur_length = value(&flicker.softness).max(0.0);
        }
        VideoEffect::CineonConverter(cineon) => {
            pixel.cineon = Some(Cineon {
                conversion: match cineon.conversion {
                    CineonConversion::LogToLinear => CineonKind::LogToLinear,
                    CineonConversion::LinearToLog => CineonKind::LinearToLog,
                    CineonConversion::LogToLog => CineonKind::LogToLog,
                },
                black_code: value(&cineon.black_point_10_bit),
                white_code: value(&cineon.white_point_10_bit),
                internal_black: percent(&cineon.internal_black_point),
                internal_white: percent(&cineon.internal_white_point),
                gamma: value(&cineon.gamma),
                highlight_rolloff: percent(&cineon.highlight_rolloff),
            });
        }
        VideoEffect::BrushStrokes(brush) => {
            pixel.paint.brush = Some(Brush {
                angle: value(&brush.stroke_angle).to_radians(),
                size: value(&brush.brush_size),
                length: value(&brush.stroke_length),
                randomness: percent(&brush.stroke_randomness),
                surface: match brush.paint_surface {
                    PaintSurface::OnOriginalImage => BrushSurface::Original,
                    PaintSurface::OnTransparent => BrushSurface::Transparent,
                    PaintSurface::OnWhite => BrushSurface::White,
                    PaintSurface::OnBlack => BrushSurface::Black,
                },
                blend: percent(&brush.blend_with_original),
            });
        }
        VideoEffect::EyedropperFill(fill) => {
            pixel.paint.eyedropper = Some(Eyedropper {
                point: fill.sample_point.each_ref().map(percent),
                radius: value(&fill.sample_radius),
                keep_alpha: fill.maintain_original_alpha,
                blend: percent(&fill.blend_with_original),
            });
        }
        VideoEffect::SdrConform(conform) => {
            pixel.paint.conform = Some(Conform {
                brightness: percent(&conform.brightness),
                contrast: percent(&conform.contrast),
                knee: percent(&conform.soft_knee),
            });
        }
        _ => {}
    }
}
