use render::{
    Cdl, ColorReplace, Convolution, ExtractRange, LimitBy, Limiter, ShadowHighlight, Tone,
};
use timeline::{LimiterMethod, Param, Ticks, VideoEffect};

use crate::pixel_effects::{encoded, linear};

/// Sets Convolution Kernel, Extract, Shadow/Highlight, Video Limiter, ASC CDL or Color
/// Replace in `tone`, leaving other effects alone.
pub(crate) fn set_tone_effect(tone: &mut Tone, effect: &VideoEffect, time: Ticks) {
    let value = |param: &Param| param.value_at(time);
    let percent = |param: &Param| value(param) / 100.0;
    let level = |param: &Param| value(param) / 255.0;
    match effect {
        VideoEffect::ConvolutionKernel(convolution) => {
            // Premiere treats a zero scale as one rather than dividing by it.
            let scale = Some(value(&convolution.scale))
                .filter(|scale| *scale != 0.0)
                .unwrap_or(1.0);
            tone.convolution = Some(Convolution {
                kernel: convolution
                    .kernel
                    .each_ref()
                    .map(|weight| value(weight) / scale),
                offset: level(&convolution.offset),
            });
        }
        VideoEffect::Extract(extract) => {
            tone.extract = Some(ExtractRange {
                black: level(&extract.black_input_level),
                white: level(&extract.white_input_level),
                softness: level(&extract.softness),
                invert: extract.invert,
            });
        }
        VideoEffect::ShadowHighlight(lift) => {
            tone.shadow_highlight = Some(ShadowHighlight {
                shadow: percent(&lift.shadow_amount),
                highlight: percent(&lift.highlight_amount),
                shadow_width: percent(&lift.shadow_tonal_width),
                highlight_width: percent(&lift.highlight_tonal_width),
                radius: value(&lift.radius),
                midtone_contrast: percent(&lift.midtone_contrast),
                blend: percent(&lift.blend_with_original),
            });
        }
        VideoEffect::VideoLimiter(limiter) => {
            tone.limiter = Some(Limiter {
                level: percent(&limiter.clip_level),
                knee: percent(&limiter.compression_before_clipping),
                by: match limiter.method {
                    LimiterMethod::ReduceLuma => LimitBy::Luma,
                    LimiterMethod::ReduceChroma => LimitBy::Chroma,
                    LimiterMethod::ReduceChromaAndLuma => LimitBy::ChromaAndLuma,
                    LimiterMethod::SmartLimit => LimitBy::Smart,
                },
                warning: limiter.gamut_warning.map(linear),
            });
        }
        VideoEffect::AscCdl(cdl) => {
            tone.cdl = Some(Cdl {
                slope: cdl.slope.each_ref().map(value),
                offset: cdl.offset.each_ref().map(value),
                power: cdl.power.each_ref().map(value),
                saturation: value(&cdl.saturation),
            });
        }
        VideoEffect::ColorReplace(replace) => {
            tone.color_replace = Some(ColorReplace {
                target: encoded(replace.target_color),
                replacement: encoded(replace.replace_color),
                similarity: percent(&replace.similarity),
                solid: replace.solid_colors,
            });
        }
        _ => {}
    }
}
