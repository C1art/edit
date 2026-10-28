use super::{
    ColorChange, ColorMatch, EffectCategory, KeyOutput, MatteChannel, RampShape, Rgb,
    TimecodeSource, VideoEffect,
};
use crate::{Param, Stabilizer};

impl VideoEffect {
    /// Each effect with its default settings, in the order the effects menu lists them.
    pub fn catalog() -> [Self; 44] {
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
                output: KeyOutput::Composite,
                pedestal: Param::from_value(10.0),
                contrast: Param::default(),
                choke: Param::default(),
                soften: Param::default(),
                spill: Param::from_value(50.0),
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
            Self::ColorKey {
                color: Rgb::GREEN,
                tolerance: Param::from_value(20.0),
                softness: Param::from_value(5.0),
            },
            Self::LeaveColor {
                color: Rgb::new(255, 0, 0),
                amount: Param::from_value(100.0),
                tolerance: Param::from_value(15.0),
                softness: Param::from_value(5.0),
                matching: ColorMatch::Hue,
            },
            Self::GradientRamp {
                start: [Param::from_value(50.0), Param::default()],
                start_color: Rgb::BLACK,
                end: [Param::from_value(50.0), Param::from_value(100.0)],
                end_color: Rgb::WHITE,
                shape: RampShape::Linear,
                blend: Param::default(),
            },
            Self::Mirror {
                center: [Param::from_value(50.0), Param::from_value(50.0)],
                angle: Param::default(),
            },
            Self::CornerPin {
                corners: [[0.0, 0.0], [100.0, 0.0], [0.0, 100.0], [100.0, 100.0]]
                    .map(|corner| corner.map(Param::from_value)),
            },
            Self::PosterizeTime {
                frame_rate: Param::from_value(12.0),
            },
            Self::StrobeLight {
                color: Rgb::WHITE,
                blend: Param::from_value(50.0),
                duration: Param::from_value(0.1),
                period: Param::from_value(1.0),
            },
            Self::Timecode {
                position: [Param::from_value(50.0), Param::from_value(90.0)],
                size: Param::from_value(6.0),
                opacity: Param::from_value(60.0),
                source: TimecodeSource::Media,
            },
            Self::Transform {
                anchor: [Param::from_value(50.0), Param::from_value(50.0)],
                position: [Param::from_value(50.0), Param::from_value(50.0)],
                scale: [Param::from_value(100.0), Param::from_value(100.0)],
                skew: Param::default(),
                skew_axis: Param::default(),
                rotation: Param::default(),
                opacity: Param::from_value(100.0),
            },
            Self::Basic3D {
                swivel: Param::default(),
                tilt: Param::default(),
                distance: Param::default(),
            },
            Self::Spherize {
                radius: Param::from_value(200.0),
                center: [Param::from_value(50.0), Param::from_value(50.0)],
            },
            Self::Emboss {
                direction: Param::from_value(45.0),
                relief: Param::from_value(2.0),
                contrast: Param::from_value(100.0),
                blend: Param::default(),
            },
            Self::TurbulentDisplace {
                amount: Param::from_value(50.0),
                size: Param::from_value(100.0),
                offset: [Param::from_value(50.0), Param::from_value(50.0)],
                complexity: Param::from_value(1.0),
                evolution: Param::default(),
            },
            Self::ChannelMixer {
                red: [100.0, 0.0, 0.0, 0.0].map(Param::from_value),
                green: [0.0, 100.0, 0.0, 0.0].map(Param::from_value),
                blue: [0.0, 0.0, 100.0, 0.0].map(Param::from_value),
                monochrome: false,
            },
            Self::ChangeToColor {
                from: Rgb::new(255, 0, 0),
                to: Rgb::new(0, 0, 255),
                change: ColorChange::Hue,
                tolerance: [5.0, 50.0, 50.0].map(Param::from_value),
                softness: Param::from_value(50.0),
            },
            Self::LumaKey {
                threshold: Param::from_value(25.0),
                cutoff: Param::default(),
            },
            Self::AlphaAdjust {
                opacity: Param::from_value(100.0),
                ignore_alpha: false,
                invert_alpha: false,
                mask_only: false,
            },
            Self::Solarize {
                threshold: Param::from_value(50.0),
            },
            Self::Threshold {
                level: Param::from_value(128.0),
            },
            Self::Replicate {
                count: Param::from_value(2.0),
            },
            Self::ColorBalanceRgb {
                gains: [100.0; 3].map(Param::from_value),
            },
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
            | Self::WarpStabilizer(_)
            | Self::Mirror { .. }
            | Self::CornerPin { .. }
            | Self::Transform { .. }
            | Self::Spherize { .. }
            | Self::TurbulentDisplace { .. } => EffectCategory::Distort,
            Self::LeaveColor { .. } | Self::ChannelMixer { .. } | Self::ChangeToColor { .. } => {
                EffectCategory::ColorCorrection
            }
            Self::GradientRamp { .. } => EffectCategory::Generate,
            Self::DropShadow { .. } | Self::Basic3D { .. } => EffectCategory::Perspective,
            Self::BlackAndWhite | Self::GammaCorrection { .. } | Self::ColorBalanceRgb { .. } => {
                EffectCategory::ImageControl
            }
            Self::UltraKey { .. }
            | Self::TrackMatteKey { .. }
            | Self::ColorKey { .. }
            | Self::LumaKey { .. }
            | Self::AlphaAdjust { .. } => EffectCategory::Keying,
            Self::Invert { .. }
            | Self::Vignette { .. }
            | Self::Posterize { .. }
            | Self::Noise { .. }
            | Self::FindEdges { .. }
            | Self::Mosaic { .. }
            | Self::StrobeLight { .. }
            | Self::Emboss { .. }
            | Self::Solarize { .. }
            | Self::Threshold { .. }
            | Self::Replicate { .. } => EffectCategory::Stylize,
            Self::PosterizeTime { .. } => EffectCategory::Time,
            Self::Timecode { .. } => EffectCategory::Video,
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
            Self::ColorKey { .. } => "Color Key",
            Self::LeaveColor { .. } => "Leave Color",
            Self::GradientRamp { .. } => "Gradient Ramp",
            Self::Mirror { .. } => "Mirror",
            Self::CornerPin { .. } => "Corner Pin",
            Self::PosterizeTime { .. } => "Posterize Time",
            Self::StrobeLight { .. } => "Strobe Light",
            Self::Timecode { .. } => "Timecode",
            Self::Transform { .. } => "Transform",
            Self::Basic3D { .. } => "Basic 3D",
            Self::Spherize { .. } => "Spherize",
            Self::Emboss { .. } => "Emboss",
            Self::TurbulentDisplace { .. } => "Turbulent Displace",
            Self::ChannelMixer { .. } => "Channel Mixer",
            Self::ChangeToColor { .. } => "Change to Color",
            Self::LumaKey { .. } => "Luma Key",
            Self::AlphaAdjust { .. } => "Alpha Adjust",
            Self::Solarize { .. } => "Solarize",
            Self::Threshold { .. } => "Threshold",
            Self::Replicate { .. } => "Replicate",
            Self::ColorBalanceRgb { .. } => "Color Balance (RGB)",
        }
    }
}
