// Expanded inside `effect_params!`, whose file brings the effect types into scope.
/// The parameters of the effects `effect_params!` hands on, kept apart so that one
/// file doesn't hold them all.
macro_rules! more_effect_params {
    ($effect:expr, $iter:ident) => {
        match $effect {
            VideoEffect::ColorPass(ColorPass { similarity, .. }) => vec![similarity],
            VideoEffect::ReduceInterlaceFlicker(ReduceInterlaceFlicker { softness }) => {
                vec![softness]
            }
            VideoEffect::CineonConverter(CineonConverter {
                black_point_10_bit,
                internal_black_point,
                white_point_10_bit,
                internal_white_point,
                gamma,
                highlight_rolloff,
                ..
            }) => vec![
                black_point_10_bit,
                internal_black_point,
                white_point_10_bit,
                internal_white_point,
                gamma,
                highlight_rolloff,
            ],
            VideoEffect::NoiseAlpha(NoiseAlpha {
                amount,
                noise_phase,
                cycle,
                ..
            }) => vec![amount, noise_phase, cycle],
            VideoEffect::ChannelBlur(ChannelBlur {
                red,
                green,
                blue,
                alpha,
            }) => vec![red, green, blue, alpha],
            VideoEffect::CameraBlur { percent } => vec![percent],
            VideoEffect::Echo(Echo {
                echo_time,
                echoes,
                starting_intensity,
                decay,
                operator: _,
            }) => vec![echo_time, echoes, starting_intensity, decay],
            VideoEffect::GarbageMatte(matte) => matte.points.$iter().flatten().collect(),
            VideoEffect::AutoReframe(AutoReframe {
                offset: [x, y],
                scale,
                ..
            }) => vec![x, y, scale],
            VideoEffect::RoughenEdges(RoughenEdges {
                border,
                edge_sharpness,
                fractal_influence,
                scale,
                offset: [x, y],
                complexity,
                evolution,
                ..
            }) => vec![
                border,
                edge_sharpness,
                fractal_influence,
                scale,
                x,
                y,
                complexity,
                evolution,
            ],
            VideoEffect::BevelEdges(Bevel {
                edge_thickness,
                light_angle,
                light_intensity,
                ..
            })
            | VideoEffect::BevelAlpha(Bevel {
                edge_thickness,
                light_angle,
                light_intensity,
                ..
            }) => vec![edge_thickness, light_angle, light_intensity],
            VideoEffect::AlphaGlow(AlphaGlow {
                glow, brightness, ..
            }) => vec![glow, brightness],
            VideoEffect::ColorEmboss(ColorEmboss {
                direction,
                relief,
                contrast,
                blend,
            }) => vec![direction, relief, contrast, blend],
            VideoEffect::ConvolutionKernel(ConvolutionKernel {
                kernel,
                offset,
                scale,
            }) => kernel.$iter().chain([offset, scale]).collect(),
            VideoEffect::Extract(Extract {
                black_input_level,
                white_input_level,
                softness,
                invert: _,
            }) => vec![black_input_level, white_input_level, softness],
            VideoEffect::ShadowHighlight(ShadowHighlight {
                shadow_amount,
                highlight_amount,
                shadow_tonal_width,
                highlight_tonal_width,
                radius,
                midtone_contrast,
                blend_with_original,
            }) => vec![
                shadow_amount,
                highlight_amount,
                shadow_tonal_width,
                highlight_tonal_width,
                radius,
                midtone_contrast,
                blend_with_original,
            ],
            VideoEffect::VideoLimiter(VideoLimiter {
                clip_level,
                compression_before_clipping,
                ..
            }) => vec![clip_level, compression_before_clipping],
            VideoEffect::AscCdl(AscCdl {
                slope,
                offset,
                power,
                saturation,
            }) => slope
                .$iter()
                .chain(offset.$iter())
                .chain(power.$iter())
                .chain([saturation])
                .collect(),
            VideoEffect::ColorReplace(ColorReplace { similarity, .. }) => vec![similarity],
            VideoEffect::ChangeColor(ChangeColor {
                hue_transform,
                lightness_transform,
                saturation_transform,
                matching_tolerance,
                matching_softness,
                ..
            }) => vec![
                hue_transform,
                lightness_transform,
                saturation_transform,
                matching_tolerance,
                matching_softness,
            ],
            VideoEffect::ColorBalance(ColorBalance {
                shadows,
                midtones,
                highlights,
                preserve_luminosity: _,
            }) => shadows
                .$iter()
                .chain(midtones.$iter())
                .chain(highlights.$iter())
                .collect(),
            VideoEffect::BrushStrokes(BrushStrokes {
                stroke_angle,
                brush_size,
                stroke_length,
                stroke_randomness,
                paint_surface: _,
                blend_with_original,
            }) => vec![
                stroke_angle,
                brush_size,
                stroke_length,
                stroke_randomness,
                blend_with_original,
            ],
            VideoEffect::EyedropperFill(EyedropperFill {
                sample_point: [x, y],
                sample_radius,
                maintain_original_alpha: _,
                blend_with_original,
            }) => vec![x, y, sample_radius, blend_with_original],
            VideoEffect::SdrConform(SdrConform {
                brightness,
                contrast,
                soft_knee,
            }) => vec![brightness, contrast, soft_knee],
            // `effect_params!` forwards only the variants handled above.
            _ => Vec::new(),
        }
    };
}
