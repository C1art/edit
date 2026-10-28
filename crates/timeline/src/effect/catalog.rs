use super::{EffectCategory, MatteChannel, Rgb, VideoEffect};
use crate::{Param, Stabilizer};

impl VideoEffect {
    /// Each effect with its default settings, in the order the effects menu lists them.
    pub fn catalog() -> [Self; 23] {
        [
            Self::GaussianBlur {
                blurriness: Param::from_value(10.0),
            },
            Self::HorizontalFlip,
            Self::VerticalFlip,
            Self::BlackAndWhite,
            Self::Tint {
                black: Rgb::BLACK,
                white: Rgb::WHITE,
                amount: Param::from_value(100.0),
            },
            Self::Invert {
                amount: Param::from_value(100.0),
            },
            Self::Sharpen {
                amount: Param::from_value(50.0),
            },
            Self::Vignette {
                amount: Param::from_value(-50.0),
                midpoint: Param::from_value(50.0),
                feather: Param::from_value(50.0),
            },
            Self::UltraKey {
                color: Rgb::GREEN,
                tolerance: Param::from_value(30.0),
                softness: Param::from_value(10.0),
            },
            Self::TrackMatteKey {
                matte: None,
                channel: MatteChannel::Alpha,
                reverse: false,
            },
            Self::BrightnessContrast {
                brightness: Param::default(),
                contrast: Param::default(),
            },
            Self::GammaCorrection {
                gamma: Param::from_value(1.0),
            },
            Self::Posterize {
                levels: Param::from_value(7.0),
            },
            Self::Noise {
                amount: Param::from_value(20.0),
            },
            Self::FindEdges {
                invert: false,
                blend: Param::default(),
            },
            Self::Mosaic {
                horizontal: Param::from_value(10.0),
                vertical: Param::from_value(10.0),
            },
            Self::Offset {
                shift: [Param::from_value(50.0), Param::from_value(50.0)],
                blend: Param::default(),
            },
            Self::Twirl {
                angle: Param::from_value(50.0),
                radius: Param::from_value(75.0),
            },
            Self::LensDistortion {
                curvature: Param::from_value(-30.0),
            },
            Self::DirectionalBlur {
                direction: Param::default(),
                length: Param::from_value(10.0),
            },
            Self::DropShadow {
                color: Rgb::BLACK,
                opacity: Param::from_value(50.0),
                direction: Param::from_value(135.0),
                distance: Param::from_value(10.0),
                softness: Param::from_value(10.0),
            },
            Self::WaveWarp {
                height: Param::from_value(10.0),
                width: Param::from_value(40.0),
                direction: Param::from_value(90.0),
                speed: Param::from_value(1.0),
            },
            Self::WarpStabilizer(Stabilizer::default()),
        ]
    }

    pub fn category(&self) -> EffectCategory {
        match self {
            Self::GaussianBlur { .. } | Self::Sharpen { .. } | Self::DirectionalBlur { .. } => {
                EffectCategory::BlurAndSharpen
            }
            Self::Tint { .. } | Self::BrightnessContrast { .. } => EffectCategory::ColorCorrection,
            Self::Twirl { .. }
            | Self::LensDistortion { .. }
            | Self::Offset { .. }
            | Self::WaveWarp { .. }
            | Self::WarpStabilizer(_) => EffectCategory::Distort,
            Self::DropShadow { .. } => EffectCategory::Perspective,
            Self::BlackAndWhite | Self::GammaCorrection { .. } => EffectCategory::ImageControl,
            Self::UltraKey { .. } | Self::TrackMatteKey { .. } => EffectCategory::Keying,
            Self::Invert { .. }
            | Self::Vignette { .. }
            | Self::Posterize { .. }
            | Self::Noise { .. }
            | Self::FindEdges { .. }
            | Self::Mosaic { .. } => EffectCategory::Stylize,
            Self::HorizontalFlip | Self::VerticalFlip => EffectCategory::Transform,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::GaussianBlur { .. } => "Gaussian Blur",
            Self::HorizontalFlip => "Horizontal Flip",
            Self::VerticalFlip => "Vertical Flip",
            Self::BlackAndWhite => "Black & White",
            Self::Tint { .. } => "Tint",
            Self::Invert { .. } => "Invert",
            Self::Sharpen { .. } => "Sharpen",
            Self::Vignette { .. } => "Vignette",
            Self::UltraKey { .. } => "Ultra Key",
            Self::TrackMatteKey { .. } => "Track Matte Key",
            Self::BrightnessContrast { .. } => "Brightness & Contrast",
            Self::GammaCorrection { .. } => "Gamma Correction",
            Self::Posterize { .. } => "Posterize",
            Self::Noise { .. } => "Noise",
            Self::FindEdges { .. } => "Find Edges",
            Self::Mosaic { .. } => "Mosaic",
            Self::Offset { .. } => "Offset",
            Self::Twirl { .. } => "Twirl",
            Self::LensDistortion { .. } => "Lens Distortion",
            Self::DirectionalBlur { .. } => "Directional Blur",
            Self::DropShadow { .. } => "Drop Shadow",
            Self::WaveWarp { .. } => "Wave Warp",
            Self::WarpStabilizer(_) => "Warp Stabilizer",
        }
    }
}
