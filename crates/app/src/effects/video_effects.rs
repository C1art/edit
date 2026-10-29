use timeline::{
    ClipNameSource, ColorChange, ColorMatch, EffectCategory, RampShape, TimecodeSource, VideoEffect,
};

use super::look::Rows;

/// Premiere's effect categories, each a submenu of the effects in it.
pub(super) fn add_menu(ui: &mut egui::Ui, rows: &mut Rows, effects: &mut Vec<VideoEffect>) {
    ui.menu_button("Add Effect", |ui| {
        for category in EffectCategory::ALL {
            ui.menu_button(category.name(), |ui| {
                let catalog = VideoEffect::catalog();
                for effect in catalog.into_iter().filter(|e| e.category() == category) {
                    if ui.button(effect.name()).clicked() {
                        // Premiere analyzes as soon as the stabilizer is applied.
                        rows.output.analyze_motion |= matches!(
                            effect,
                            VideoEffect::WarpStabilizer(_) | VideoEffect::RollingShutterRepair(_)
                        );
                        rows.output.analyze_reframe |=
                            matches!(effect, VideoEffect::AutoReframe(_));
                        effects.push(effect);
                        rows.commit();
                        ui.close();
                    }
                }
            });
        }
    });
}

impl Rows {
    pub(super) fn effect(
        &mut self,
        ui: &mut egui::Ui,
        effect: &mut VideoEffect,
        video_tracks: usize,
    ) {
        match effect {
            VideoEffect::GaussianBlur { blurriness } => {
                self.param(ui, "Blurriness", blurriness, 0.0..=500.0, "");
            }
            VideoEffect::HorizontalFlip
            | VideoEffect::VerticalFlip
            | VideoEffect::BlackAndWhite => {}
            VideoEffect::Tint {
                black,
                white,
                amount,
            } => {
                self.color(ui, "Map Black To", black);
                self.color(ui, "Map White To", white);
                self.param(ui, "Amount", amount, 0.0..=100.0, "%");
            }
            VideoEffect::Invert { amount } => {
                self.param(ui, "Blend", amount, 0.0..=100.0, "%");
            }
            VideoEffect::BrightnessContrast {
                brightness,
                contrast,
            } => {
                self.param(ui, "Brightness", brightness, -100.0..=100.0, "");
                self.param(ui, "Contrast", contrast, -100.0..=100.0, "");
            }
            VideoEffect::GammaCorrection { gamma } => {
                self.param(ui, "Gamma", gamma, 0.1..=5.0, "");
            }
            VideoEffect::Posterize { levels } => {
                self.param(ui, "Level", levels, 2.0..=64.0, "");
            }
            VideoEffect::Noise { amount } => {
                self.param(ui, "Amount", amount, 0.0..=100.0, "%");
            }
            VideoEffect::FindEdges { invert, blend } => {
                self.choice(ui, "Invert", |ui| ui.checkbox(invert, "").changed());
                self.param(ui, "Blend", blend, 0.0..=100.0, "%");
            }
            VideoEffect::Mosaic {
                horizontal,
                vertical,
            } => {
                self.param(ui, "Horizontal", horizontal, 1.0..=4_000.0, "");
                self.param(ui, "Vertical", vertical, 1.0..=4_000.0, "");
            }
            VideoEffect::Offset {
                shift: [x, y],
                blend,
            } => {
                self.param(ui, "Center X", x, -100.0..=200.0, "%");
                self.param(ui, "Center Y", y, -100.0..=200.0, "%");
                self.param(ui, "Blend", blend, 0.0..=100.0, "%");
            }
            VideoEffect::Twirl { angle, radius } => {
                self.param(ui, "Angle", angle, -3_600.0..=3_600.0, "°");
                self.param(ui, "Radius", radius, 0.0..=100.0, "%");
            }
            VideoEffect::LensDistortion { curvature } => {
                self.param(ui, "Curvature", curvature, -100.0..=100.0, "");
            }
            VideoEffect::DirectionalBlur { direction, length } => {
                self.param(ui, "Direction", direction, -360.0..=360.0, "°");
                self.param(ui, "Length", length, 0.0..=500.0, " px");
            }
            VideoEffect::DropShadow {
                color,
                opacity,
                direction,
                distance,
                softness,
            } => {
                self.color(ui, "Shadow Color", color);
                self.param(ui, "Opacity", opacity, 0.0..=100.0, "%");
                self.param(ui, "Direction", direction, -360.0..=360.0, "°");
                self.param(ui, "Distance", distance, 0.0..=1_000.0, " px");
                self.param(ui, "Softness", softness, 0.0..=250.0, " px");
            }
            VideoEffect::WaveWarp {
                height,
                width,
                direction,
                speed,
            } => {
                self.param(ui, "Wave Height", height, -500.0..=500.0, " px");
                self.param(ui, "Wave Width", width, 1.0..=2_000.0, " px");
                self.param(ui, "Direction", direction, -360.0..=360.0, "°");
                self.param(ui, "Wave Speed", speed, -10.0..=10.0, "/s");
            }
            VideoEffect::PosterizeTime { frame_rate } => {
                self.param(ui, "Frame Rate", frame_rate, 1.0..=60.0, " fps");
            }
            VideoEffect::StrobeLight {
                color,
                blend,
                duration,
                period,
            } => {
                self.color(ui, "Strobe Color", color);
                self.param(ui, "Amount", blend, 0.0..=100.0, "%");
                self.param(ui, "Strobe Duration", duration, 0.0..=10.0, " s");
                self.param(ui, "Strobe Period", period, 0.01..=10.0, " s");
            }
            VideoEffect::Timecode {
                position: [x, y],
                size,
                opacity,
                source,
            } => {
                self.param(ui, "Position X", x, 0.0..=100.0, "%");
                self.param(ui, "Position Y", y, 0.0..=100.0, "%");
                self.param(ui, "Size", size, 1.0..=50.0, "%");
                self.param(ui, "Opacity", opacity, 0.0..=100.0, "%");
                self.options(
                    ui,
                    "Timecode Source",
                    "timecode_source",
                    source,
                    TimecodeSource::ALL,
                    TimecodeSource::name,
                );
            }
            VideoEffect::ClipName {
                position: [x, y],
                size,
                opacity,
                source,
            } => {
                self.param(ui, "Position X", x, 0.0..=100.0, "%");
                self.param(ui, "Position Y", y, 0.0..=100.0, "%");
                self.param(ui, "Size", size, 1.0..=50.0, "%");
                self.param(ui, "Opacity", opacity, 0.0..=100.0, "%");
                self.options(
                    ui,
                    "Display",
                    "clip_name_source",
                    source,
                    ClipNameSource::ALL,
                    ClipNameSource::name,
                );
            }
            VideoEffect::Transform {
                anchor: [anchor_x, anchor_y],
                position: [x, y],
                scale: [width, height],
                skew,
                skew_axis,
                rotation,
                opacity,
            } => {
                self.param(ui, "Anchor Point X", anchor_x, -100.0..=200.0, "%");
                self.param(ui, "Anchor Point Y", anchor_y, -100.0..=200.0, "%");
                self.param(ui, "Position X", x, -100.0..=200.0, "%");
                self.param(ui, "Position Y", y, -100.0..=200.0, "%");
                self.param(ui, "Scale Width", width, 0.0..=600.0, "%");
                self.param(ui, "Scale Height", height, 0.0..=600.0, "%");
                self.param(ui, "Skew", skew, -85.0..=85.0, "°");
                self.param(ui, "Skew Axis", skew_axis, -360.0..=360.0, "°");
                self.param(ui, "Rotation", rotation, -360.0..=360.0, "°");
                self.param(ui, "Opacity", opacity, 0.0..=100.0, "%");
            }
            VideoEffect::Basic3D {
                swivel,
                tilt,
                distance,
            } => {
                self.param(ui, "Swivel", swivel, -90.0..=90.0, "°");
                self.param(ui, "Tilt", tilt, -90.0..=90.0, "°");
                self.param(ui, "Distance to Image", distance, -99.0..=500.0, "%");
            }
            VideoEffect::Spherize {
                radius,
                center: [x, y],
            } => {
                self.param(ui, "Radius", radius, 0.0..=2500.0, "");
                self.param(ui, "Center X", x, -100.0..=200.0, "%");
                self.param(ui, "Center Y", y, -100.0..=200.0, "%");
            }
            VideoEffect::Emboss {
                direction,
                relief,
                contrast,
                blend,
            } => {
                self.param(ui, "Direction", direction, -360.0..=360.0, "°");
                self.param(ui, "Relief", relief, 1.0..=10.0, "");
                self.param(ui, "Contrast", contrast, 0.0..=500.0, "%");
                self.param(ui, "Blend With Original", blend, 0.0..=100.0, "%");
            }
            VideoEffect::TurbulentDisplace {
                amount,
                size,
                offset: [x, y],
                complexity,
                evolution,
            } => {
                self.param(ui, "Amount", amount, -1000.0..=1000.0, "");
                self.param(ui, "Size", size, 2.0..=1000.0, "");
                self.param(ui, "Offset X", x, -100.0..=200.0, "%");
                self.param(ui, "Offset Y", y, -100.0..=200.0, "%");
                self.param(ui, "Complexity", complexity, 1.0..=10.0, "");
                self.param(ui, "Evolution", evolution, -3600.0..=3600.0, "°");
            }
            VideoEffect::ChannelMixer {
                red,
                green,
                blue,
                monochrome,
            } => {
                let sources = ["Red", "Green", "Blue", "Const"];
                for (output, row) in [("Red", red), ("Green", green), ("Blue", blue)] {
                    for (source, share) in sources.iter().zip(row) {
                        let label = format!("{output}-{source}");
                        self.param(ui, &label, share, -200.0..=200.0, "");
                    }
                }
                self.choice(ui, "Monochrome", |ui| ui.checkbox(monochrome, "").changed());
            }
            VideoEffect::ChangeToColor {
                from,
                to,
                change,
                tolerance: [hue, lightness, saturation],
                softness,
            } => {
                self.color(ui, "From", from);
                self.color(ui, "To", to);
                self.options(
                    ui,
                    "Change",
                    "change_to_color",
                    change,
                    ColorChange::ALL,
                    ColorChange::name,
                );
                self.param(ui, "Hue Tolerance", hue, 0.0..=100.0, "%");
                self.param(ui, "Lightness Tolerance", lightness, 0.0..=100.0, "%");
                self.param(ui, "Saturation Tolerance", saturation, 0.0..=100.0, "%");
                self.param(ui, "Softness", softness, 0.0..=100.0, "%");
            }
            VideoEffect::Sharpen { amount } => {
                self.param(ui, "Amount", amount, 0.0..=500.0, "");
            }
            VideoEffect::Vignette {
                amount,
                midpoint,
                feather,
            } => {
                self.param(ui, "Amount", amount, -100.0..=100.0, "");
                self.param(ui, "Midpoint", midpoint, 0.0..=100.0, "");
                self.param(ui, "Feather", feather, 0.0..=100.0, "");
            }
            VideoEffect::WarpStabilizer(stabilizer) => self.stabilizer(ui, stabilizer),
            VideoEffect::RollingShutterRepair(repair) => self.rolling_shutter(ui, repair),
            VideoEffect::AutoReframe(reframe) => self.auto_reframe(ui, reframe),
            VideoEffect::UltraKey { .. }
            | VideoEffect::TrackMatteKey { .. }
            | VideoEffect::SetMatte { .. }
            | VideoEffect::ColorKey { .. }
            | VideoEffect::LumaKey { .. }
            | VideoEffect::AlphaAdjust { .. }
            | VideoEffect::GarbageMatte(_) => self.keying(ui, effect, video_tracks),
            VideoEffect::DifferenceMatte(_)
            | VideoEffect::CompoundBlur(_)
            | VideoEffect::Texturize(_) => self.layered(ui, effect, video_tracks),
            VideoEffect::Checkerboard(_)
            | VideoEffect::Grid(_)
            | VideoEffect::Circle(_)
            | VideoEffect::FourColorGradient(_)
            | VideoEffect::Ellipse(_)
            | VideoEffect::CellPattern(_)
            | VideoEffect::Lightning(_) => self.generate(ui, effect),
            VideoEffect::LightingEffects(lighting) => self.lighting_effects(ui, lighting),
            VideoEffect::RadialShadow(shadow) => self.radial_shadow(ui, shadow),
            VideoEffect::ColorPass(_)
            | VideoEffect::ReduceInterlaceFlicker(_)
            | VideoEffect::CineonConverter(_) => self.image(ui, effect),
            VideoEffect::BrushStrokes(_)
            | VideoEffect::EyedropperFill(_)
            | VideoEffect::PaintBucket(_)
            | VideoEffect::WriteOn(_)
            | VideoEffect::SdrConform(_) => self.paint(ui, effect),
            VideoEffect::LensFlare { .. } | VideoEffect::Magnify { .. } => self.optics(ui, effect),
            VideoEffect::LinearWipe { .. }
            | VideoEffect::RadialWipe { .. }
            | VideoEffect::VenetianBlinds { .. }
            | VideoEffect::BlockDissolve { .. } => self.clip_wipes(ui, effect),
            VideoEffect::UnsharpMask(_)
            | VideoEffect::Median(_)
            | VideoEffect::DustAndScratches(_)
            | VideoEffect::EdgeFeather(_)
            | VideoEffect::NoiseHls(_)
            | VideoEffect::NoiseHlsAuto(_)
            | VideoEffect::NoiseAlpha(_) => self.detail(ui, effect),
            VideoEffect::RoughenEdges(_)
            | VideoEffect::BevelEdges(_)
            | VideoEffect::BevelAlpha(_)
            | VideoEffect::AlphaGlow(_)
            | VideoEffect::ColorEmboss(_) => self.edges(ui, effect),
            VideoEffect::ConvolutionKernel(_)
            | VideoEffect::Extract(_)
            | VideoEffect::ShadowHighlight(_)
            | VideoEffect::VideoLimiter(_)
            | VideoEffect::AscCdl(_)
            | VideoEffect::ColorReplace(_) => self.tone(ui, effect),
            VideoEffect::ChangeColor(_) | VideoEffect::ColorBalance(_) => self.shift(ui, effect),
            VideoEffect::SimpleText(_) => self.simple_text(ui, effect),
            VideoEffect::ChannelBlur(_) | VideoEffect::CameraBlur { .. } | VideoEffect::Echo(_) => {
                self.blur_echo(ui, effect)
            }
            VideoEffect::LeaveColor {
                color,
                amount,
                tolerance,
                softness,
                matching,
            } => {
                self.param(ui, "Amount", amount, 0.0..=100.0, "%");
                self.color(ui, "Color", color);
                self.param(ui, "Tolerance", tolerance, 0.0..=100.0, "%");
                self.param(ui, "Softness", softness, 0.0..=100.0, "%");
                self.options(
                    ui,
                    "Match",
                    "leave_match",
                    matching,
                    ColorMatch::ALL,
                    ColorMatch::name,
                );
            }
            VideoEffect::GradientRamp {
                start: [start_x, start_y],
                start_color,
                end: [end_x, end_y],
                end_color,
                shape,
                blend,
            } => {
                self.param(ui, "Start X", start_x, -100.0..=200.0, "%");
                self.param(ui, "Start Y", start_y, -100.0..=200.0, "%");
                self.color(ui, "Start Color", start_color);
                self.param(ui, "End X", end_x, -100.0..=200.0, "%");
                self.param(ui, "End Y", end_y, -100.0..=200.0, "%");
                self.color(ui, "End Color", end_color);
                self.options(
                    ui,
                    "Shape",
                    "ramp_shape",
                    shape,
                    RampShape::ALL,
                    RampShape::name,
                );
                self.param(ui, "Blend", blend, 0.0..=100.0, "%");
            }
            VideoEffect::Mirror {
                center: [x, y],
                angle,
            } => {
                self.param(ui, "Center X", x, -100.0..=200.0, "%");
                self.param(ui, "Center Y", y, -100.0..=200.0, "%");
                self.param(ui, "Angle", angle, -360.0..=360.0, "°");
            }
            VideoEffect::Solarize { threshold } => {
                self.param(ui, "Threshold", threshold, 0.0..=100.0, "%");
            }
            VideoEffect::Threshold { level } => {
                self.param(ui, "Level", level, 0.0..=255.0, "");
            }
            VideoEffect::Replicate { count } => {
                self.param(ui, "Count", count, 2.0..=16.0, "");
            }
            VideoEffect::Levels {
                input_black,
                input_white,
                output_black,
                output_white,
                gamma,
            } => {
                self.param(ui, "Input Black Level", input_black, 0.0..=255.0, "");
                self.param(ui, "Input White Level", input_white, 0.0..=255.0, "");
                self.param(ui, "Output Black Level", output_black, 0.0..=255.0, "");
                self.param(ui, "Output White Level", output_white, 0.0..=255.0, "");
                self.param(ui, "Gamma", gamma, 0.1..=10.0, "");
            }
            VideoEffect::ProcAmp {
                brightness,
                contrast,
                hue,
                saturation,
            } => {
                self.param(ui, "Brightness", brightness, -100.0..=100.0, "");
                self.param(ui, "Contrast", contrast, 0.0..=200.0, "");
                self.param(ui, "Hue", hue, -180.0..=180.0, "°");
                self.param(ui, "Saturation", saturation, 0.0..=200.0, "");
            }
            VideoEffect::ColorBalanceRgb { gains } => {
                for (name, gain) in ["Red", "Green", "Blue"].into_iter().zip(gains) {
                    self.param(ui, name, gain, 0.0..=200.0, "%");
                }
            }
            VideoEffect::CornerPin { corners } => {
                let names = ["Upper Left", "Upper Right", "Lower Left", "Lower Right"];
                for (name, [x, y]) in names.into_iter().zip(corners) {
                    self.param(ui, &format!("{name} X"), x, -200.0..=300.0, "%");
                    self.param(ui, &format!("{name} Y"), y, -200.0..=300.0, "%");
                }
            }
        }
    }
}
