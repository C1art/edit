use super::{ChannelBlur, Echo, EdgeFeather, Median, NoiseHls, UnsharpMask, VideoEffect};
use crate::{AutoReframe, Param};

// One match serves both borrows: default binding modes make the same patterns bind
// `&Param` or `&mut Param`, and `$iter` picks the iterator that matches.
macro_rules! effect_params {
    ($effect:expr, $iter:ident) => {
        match $effect {
            VideoEffect::GaussianBlur { blurriness } => vec![blurriness],
            VideoEffect::GammaCorrection { gamma: one }
            | VideoEffect::Posterize { levels: one }
            | VideoEffect::PosterizeTime { frame_rate: one }
            | VideoEffect::Noise { amount: one }
            | VideoEffect::FindEdges { blend: one, .. }
            | VideoEffect::LensDistortion { curvature: one } => vec![one],
            VideoEffect::BrightnessContrast {
                brightness: first,
                contrast: second,
            }
            | VideoEffect::Mosaic {
                horizontal: first,
                vertical: second,
            }
            | VideoEffect::Twirl {
                angle: first,
                radius: second,
            }
            | VideoEffect::DirectionalBlur {
                direction: first,
                length: second,
            } => vec![first, second],
            VideoEffect::Offset {
                shift: [x, y],
                blend,
            } => vec![x, y, blend],
            VideoEffect::HorizontalFlip
            | VideoEffect::VerticalFlip
            | VideoEffect::BlackAndWhite
            | VideoEffect::TrackMatteKey { .. }
            | VideoEffect::WarpStabilizer(_) => Vec::new(),
            VideoEffect::Tint { amount, .. }
            | VideoEffect::Invert { amount }
            | VideoEffect::Sharpen { amount } => {
                vec![amount]
            }
            VideoEffect::Vignette {
                amount,
                midpoint,
                feather,
            } => vec![amount, midpoint, feather],
            VideoEffect::UltraKey {
                tolerance,
                softness,
                pedestal,
                contrast,
                choke,
                soften,
                spill,
                ..
            } => vec![
                tolerance, softness, pedestal, contrast, choke, soften, spill,
            ],
            VideoEffect::DropShadow {
                opacity,
                direction,
                distance,
                softness,
                ..
            } => vec![opacity, direction, distance, softness],
            VideoEffect::ColorKey {
                tolerance,
                softness,
                ..
            } => vec![tolerance, softness],
            VideoEffect::LeaveColor {
                amount,
                tolerance,
                softness,
                ..
            } => vec![amount, tolerance, softness],
            VideoEffect::GradientRamp {
                start: [start_x, start_y],
                end: [end_x, end_y],
                blend,
                ..
            } => vec![start_x, start_y, end_x, end_y, blend],
            VideoEffect::Mirror {
                center: [x, y],
                angle,
            } => vec![x, y, angle],
            VideoEffect::CornerPin { corners } => corners.$iter().flatten().collect(),
            VideoEffect::WaveWarp {
                height,
                width,
                direction,
                speed,
            } => vec![height, width, direction, speed],
            VideoEffect::StrobeLight {
                blend,
                duration,
                period,
                ..
            } => vec![blend, duration, period],
            VideoEffect::Timecode {
                position: [x, y],
                size,
                opacity,
                ..
            }
            | VideoEffect::ClipName {
                position: [x, y],
                size,
                opacity,
                ..
            } => vec![x, y, size, opacity],
            VideoEffect::Transform {
                anchor: [anchor_x, anchor_y],
                position: [x, y],
                scale: [width, height],
                skew,
                skew_axis,
                rotation,
                opacity,
            } => vec![
                anchor_x, anchor_y, x, y, width, height, skew, skew_axis, rotation, opacity,
            ],
            VideoEffect::Basic3D {
                swivel,
                tilt,
                distance,
            } => vec![swivel, tilt, distance],
            VideoEffect::Spherize {
                radius,
                center: [x, y],
            } => vec![radius, x, y],
            VideoEffect::Emboss {
                direction,
                relief,
                contrast,
                blend,
            } => vec![direction, relief, contrast, blend],
            VideoEffect::TurbulentDisplace {
                amount,
                size,
                offset: [x, y],
                complexity,
                evolution,
            } => vec![amount, size, x, y, complexity, evolution],
            VideoEffect::ChannelMixer {
                red, green, blue, ..
            } => red
                .$iter()
                .chain(green.$iter())
                .chain(blue.$iter())
                .collect(),
            VideoEffect::ChangeToColor {
                tolerance,
                softness,
                ..
            } => tolerance.$iter().chain([softness]).collect(),
            VideoEffect::Solarize { threshold: one }
            | VideoEffect::Threshold { level: one }
            | VideoEffect::Replicate { count: one } => vec![one],
            VideoEffect::LumaKey { threshold, cutoff } => vec![threshold, cutoff],
            VideoEffect::AlphaAdjust { opacity, .. } => vec![opacity],
            VideoEffect::ColorBalanceRgb { gains } => gains.$iter().collect(),
            VideoEffect::Checkerboard {
                anchor,
                size,
                opacity,
                ..
            } => anchor
                .$iter()
                .chain(size.$iter())
                .chain([opacity])
                .collect(),
            VideoEffect::Grid {
                anchor,
                size,
                border,
                opacity,
                ..
            } => anchor
                .$iter()
                .chain(size.$iter())
                .chain([border, opacity])
                .collect(),
            VideoEffect::Circle {
                center,
                radius,
                feather,
                opacity,
                ..
            } => center.$iter().chain([radius, feather, opacity]).collect(),
            VideoEffect::FourColorGradient {
                points,
                blend,
                opacity,
                ..
            } => points.$iter().flatten().chain([blend, opacity]).collect(),
            VideoEffect::LensFlare {
                center,
                brightness,
                blend,
                ..
            } => center.$iter().chain([brightness, blend]).collect(),
            VideoEffect::Magnify {
                center,
                magnification,
                size,
                feather,
                opacity,
                ..
            } => center
                .$iter()
                .chain([magnification, size, feather, opacity])
                .collect(),
            VideoEffect::LinearWipe {
                completion,
                angle,
                feather,
            } => vec![completion, angle, feather],
            VideoEffect::Levels {
                input_black,
                input_white,
                output_black,
                output_white,
                gamma,
            } => vec![input_black, input_white, output_black, output_white, gamma],
            VideoEffect::ProcAmp {
                brightness,
                contrast,
                hue,
                saturation,
            } => vec![brightness, contrast, hue, saturation],
            VideoEffect::RadialWipe {
                completion,
                start_angle,
                center,
                feather,
                ..
            } => [completion, start_angle]
                .into_iter()
                .chain(center.$iter())
                .chain([feather])
                .collect(),
            VideoEffect::VenetianBlinds {
                completion: first,
                direction: second,
                width: third,
                feather,
            }
            | VideoEffect::BlockDissolve {
                completion: first,
                block_width: second,
                block_height: third,
                feather,
            } => vec![first, second, third, feather],
            VideoEffect::UnsharpMask(UnsharpMask {
                amount,
                radius,
                threshold,
            }) => vec![amount, radius, threshold],
            VideoEffect::Median(Median { radius, threshold })
            | VideoEffect::DustAndScratches(Median { radius, threshold }) => {
                vec![radius, threshold]
            }
            VideoEffect::EdgeFeather(EdgeFeather { amount }) => vec![amount],
            VideoEffect::NoiseHls(NoiseHls {
                hue,
                lightness,
                saturation,
                grain_size,
                phase,
            }) => vec![hue, lightness, saturation, grain_size, phase],
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
        }
    };
}

impl VideoEffect {
    pub fn params_mut(&mut self) -> Vec<&mut Param> {
        effect_params!(self, iter_mut)
    }

    pub fn params(&self) -> Vec<&Param> {
        effect_params!(self, iter)
    }
}
