use super::VideoEffect;
use crate::Param;

impl VideoEffect {
    pub fn params_mut(&mut self) -> Vec<&mut Param> {
        match self {
            Self::GaussianBlur { blurriness } => vec![blurriness],
            Self::GammaCorrection { gamma: one }
            | Self::Posterize { levels: one }
            | Self::PosterizeTime { frame_rate: one }
            | Self::Noise { amount: one }
            | Self::FindEdges { blend: one, .. }
            | Self::LensDistortion { curvature: one } => vec![one],
            Self::BrightnessContrast {
                brightness: first,
                contrast: second,
            }
            | Self::Mosaic {
                horizontal: first,
                vertical: second,
            }
            | Self::Twirl {
                angle: first,
                radius: second,
            }
            | Self::DirectionalBlur {
                direction: first,
                length: second,
            } => vec![first, second],
            Self::Offset {
                shift: [x, y],
                blend,
            } => vec![x, y, blend],
            Self::HorizontalFlip
            | Self::VerticalFlip
            | Self::BlackAndWhite
            | Self::TrackMatteKey { .. }
            | Self::WarpStabilizer(_) => Vec::new(),
            Self::Tint { amount, .. } | Self::Invert { amount } | Self::Sharpen { amount } => {
                vec![amount]
            }
            Self::Vignette {
                amount,
                midpoint,
                feather,
            } => vec![amount, midpoint, feather],
            Self::UltraKey {
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
            Self::DropShadow {
                opacity,
                direction,
                distance,
                softness,
                ..
            } => vec![opacity, direction, distance, softness],
            Self::ColorKey {
                tolerance,
                softness,
                ..
            } => vec![tolerance, softness],
            Self::LeaveColor {
                amount,
                tolerance,
                softness,
                ..
            } => vec![amount, tolerance, softness],
            Self::GradientRamp {
                start: [start_x, start_y],
                end: [end_x, end_y],
                blend,
                ..
            } => vec![start_x, start_y, end_x, end_y, blend],
            Self::Mirror {
                center: [x, y],
                angle,
            } => vec![x, y, angle],
            Self::CornerPin { corners } => corners.iter_mut().flatten().collect(),
            Self::WaveWarp {
                height,
                width,
                direction,
                speed,
            } => vec![height, width, direction, speed],
            Self::StrobeLight {
                blend,
                duration,
                period,
                ..
            } => vec![blend, duration, period],
            Self::Timecode {
                position: [x, y],
                size,
                opacity,
                ..
            } => vec![x, y, size, opacity],
            Self::Transform {
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
            Self::Basic3D {
                swivel,
                tilt,
                distance,
            } => vec![swivel, tilt, distance],
            Self::Spherize {
                radius,
                center: [x, y],
            } => vec![radius, x, y],
            Self::Emboss {
                direction,
                relief,
                contrast,
                blend,
            } => vec![direction, relief, contrast, blend],
            Self::TurbulentDisplace {
                amount,
                size,
                offset: [x, y],
                complexity,
                evolution,
            } => vec![amount, size, x, y, complexity, evolution],
            Self::ChannelMixer {
                red, green, blue, ..
            } => red
                .iter_mut()
                .chain(green.iter_mut())
                .chain(blue.iter_mut())
                .collect(),
            Self::ChangeToColor {
                tolerance,
                softness,
                ..
            } => tolerance.iter_mut().chain([softness]).collect(),
        }
    }

    pub fn params(&self) -> Vec<&Param> {
        match self {
            Self::GaussianBlur { blurriness } => vec![blurriness],
            Self::GammaCorrection { gamma: one }
            | Self::Posterize { levels: one }
            | Self::PosterizeTime { frame_rate: one }
            | Self::Noise { amount: one }
            | Self::FindEdges { blend: one, .. }
            | Self::LensDistortion { curvature: one } => vec![one],
            Self::BrightnessContrast {
                brightness: first,
                contrast: second,
            }
            | Self::Mosaic {
                horizontal: first,
                vertical: second,
            }
            | Self::Twirl {
                angle: first,
                radius: second,
            }
            | Self::DirectionalBlur {
                direction: first,
                length: second,
            } => vec![first, second],
            Self::Offset {
                shift: [x, y],
                blend,
            } => vec![x, y, blend],
            Self::HorizontalFlip
            | Self::VerticalFlip
            | Self::BlackAndWhite
            | Self::TrackMatteKey { .. }
            | Self::WarpStabilizer(_) => Vec::new(),
            Self::Tint { amount, .. } | Self::Invert { amount } | Self::Sharpen { amount } => {
                vec![amount]
            }
            Self::Vignette {
                amount,
                midpoint,
                feather,
            } => vec![amount, midpoint, feather],
            Self::UltraKey {
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
            Self::DropShadow {
                opacity,
                direction,
                distance,
                softness,
                ..
            } => vec![opacity, direction, distance, softness],
            Self::ColorKey {
                tolerance,
                softness,
                ..
            } => vec![tolerance, softness],
            Self::LeaveColor {
                amount,
                tolerance,
                softness,
                ..
            } => vec![amount, tolerance, softness],
            Self::GradientRamp {
                start: [start_x, start_y],
                end: [end_x, end_y],
                blend,
                ..
            } => vec![start_x, start_y, end_x, end_y, blend],
            Self::Mirror {
                center: [x, y],
                angle,
            } => vec![x, y, angle],
            Self::CornerPin { corners } => corners.iter().flatten().collect(),
            Self::WaveWarp {
                height,
                width,
                direction,
                speed,
            } => vec![height, width, direction, speed],
            Self::StrobeLight {
                blend,
                duration,
                period,
                ..
            } => vec![blend, duration, period],
            Self::Timecode {
                position: [x, y],
                size,
                opacity,
                ..
            } => vec![x, y, size, opacity],
            Self::Transform {
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
            Self::Basic3D {
                swivel,
                tilt,
                distance,
            } => vec![swivel, tilt, distance],
            Self::Spherize {
                radius,
                center: [x, y],
            } => vec![radius, x, y],
            Self::Emboss {
                direction,
                relief,
                contrast,
                blend,
            } => vec![direction, relief, contrast, blend],
            Self::TurbulentDisplace {
                amount,
                size,
                offset: [x, y],
                complexity,
                evolution,
            } => vec![amount, size, x, y, complexity, evolution],
            Self::ChannelMixer {
                red, green, blue, ..
            } => red.iter().chain(green.iter()).chain(blue.iter()).collect(),
            Self::ChangeToColor {
                tolerance,
                softness,
                ..
            } => tolerance.iter().chain([softness]).collect(),
        }
    }
}
