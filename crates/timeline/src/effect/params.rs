use super::VideoEffect;
use crate::Param;

impl VideoEffect {
    pub fn params_mut(&mut self) -> Vec<&mut Param> {
        match self {
            Self::GaussianBlur { blurriness } => vec![blurriness],
            Self::GammaCorrection { gamma: one }
            | Self::Posterize { levels: one }
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
                ..
            } => vec![tolerance, softness],
            Self::DropShadow {
                opacity,
                direction,
                distance,
                softness,
                ..
            } => vec![opacity, direction, distance, softness],
            Self::WaveWarp {
                height,
                width,
                direction,
                speed,
            } => vec![height, width, direction, speed],
        }
    }

    pub fn params(&self) -> Vec<&Param> {
        match self {
            Self::GaussianBlur { blurriness } => vec![blurriness],
            Self::GammaCorrection { gamma: one }
            | Self::Posterize { levels: one }
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
                ..
            } => vec![tolerance, softness],
            Self::DropShadow {
                opacity,
                direction,
                distance,
                softness,
                ..
            } => vec![opacity, direction, distance, softness],
            Self::WaveWarp {
                height,
                width,
                direction,
                speed,
            } => vec![height, width, direction, speed],
        }
    }
}
