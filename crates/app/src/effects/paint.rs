use timeline::{BucketBlend, FillSelector, PaintSurface, VideoEffect, WriteOnStyle};

use super::look::Rows;

const POINT: std::ops::RangeInclusive<f32> = -100.0..=200.0;
const PERCENT: std::ops::RangeInclusive<f32> = 0.0..=100.0;
const SIGNED: std::ops::RangeInclusive<f32> = -100.0..=100.0;

impl Rows {
    /// The controls of Brush Strokes, Eyedropper Fill, Paint Bucket, Write-on and SDR
    /// Conform; other effects have none here.
    pub(super) fn paint(&mut self, ui: &mut egui::Ui, effect: &mut VideoEffect) {
        match effect {
            VideoEffect::BrushStrokes(brush) => {
                let angle = &mut brush.stroke_angle;
                self.param(ui, "Stroke Angle", angle, -360.0..=360.0, "°");
                self.param(ui, "Brush Size", &mut brush.brush_size, 0.0..=5.0, "");
                let length = &mut brush.stroke_length;
                self.param(ui, "Stroke Length", length, 0.0..=40.0, "");
                let randomness = &mut brush.stroke_randomness;
                self.param(ui, "Stroke Randomness", randomness, PERCENT, "%");
                self.options(
                    ui,
                    "Paint Surface",
                    "paint_surface",
                    &mut brush.paint_surface,
                    PaintSurface::ALL,
                    PaintSurface::name,
                );
                let blend = &mut brush.blend_with_original;
                self.param(ui, "Blend With Original", blend, PERCENT, "%");
            }
            VideoEffect::EyedropperFill(fill) => {
                let [x, y] = &mut fill.sample_point;
                self.param(ui, "Sample Point X", x, POINT, "%");
                self.param(ui, "Sample Point Y", y, POINT, "%");
                let radius = &mut fill.sample_radius;
                self.param(ui, "Sample Radius", radius, 0.0..=100.0, "px");
                let keep = &mut fill.maintain_original_alpha;
                self.choice(ui, "Maintain Original Alpha", |ui| {
                    ui.checkbox(keep, "").changed()
                });
                let blend = &mut fill.blend_with_original;
                self.param(ui, "Blend With Original", blend, PERCENT, "%");
            }
            VideoEffect::PaintBucket(bucket) => {
                let [x, y] = &mut bucket.fill_point;
                self.param(ui, "Fill Point X", x, POINT, "%");
                self.param(ui, "Fill Point Y", y, POINT, "%");
                self.options(
                    ui,
                    "Fill Selector",
                    "fill_selector",
                    &mut bucket.fill_selector,
                    FillSelector::ALL,
                    FillSelector::name,
                );
                self.param(ui, "Tolerance", &mut bucket.tolerance, 0.0..=255.0, "");
                let view = &mut bucket.view_threshold;
                self.choice(ui, "View Threshold", |ui| ui.checkbox(view, "").changed());
                self.param(ui, "Spread", &mut bucket.spread, -50.0..=50.0, "px");
                self.param(ui, "Feather", &mut bucket.feather, 0.0..=50.0, "px");
                self.color(ui, "Color", &mut bucket.color);
                self.param(ui, "Opacity", &mut bucket.opacity, PERCENT, "%");
                self.options(
                    ui,
                    "Blending Mode",
                    "bucket_blend",
                    &mut bucket.blending_mode,
                    BucketBlend::ALL,
                    BucketBlend::name,
                );
            }
            VideoEffect::WriteOn(write_on) => {
                let [x, y] = &mut write_on.brush_position;
                self.param(ui, "Brush Position X", x, POINT, "%");
                self.param(ui, "Brush Position Y", y, POINT, "%");
                self.color(ui, "Color", &mut write_on.color);
                self.param(ui, "Brush Size", &mut write_on.brush_size, 0.0..=50.0, "px");
                let hardness = &mut write_on.brush_hardness;
                self.param(ui, "Brush Hardness", hardness, PERCENT, "%");
                let opacity = &mut write_on.brush_opacity;
                self.param(ui, "Brush Opacity", opacity, PERCENT, "%");
                let length = &mut write_on.stroke_length;
                self.param(ui, "Stroke Length (secs)", length, 0.0..=30.0, "s");
                let spacing = &mut write_on.brush_spacing;
                self.param(ui, "Brush Spacing (secs)", spacing, 0.0..=1.0, "s");
                self.options(
                    ui,
                    "Paint Style",
                    "write_on_style",
                    &mut write_on.paint_style,
                    WriteOnStyle::ALL,
                    WriteOnStyle::name,
                );
            }
            VideoEffect::SdrConform(conform) => {
                self.param(ui, "Brightness", &mut conform.brightness, SIGNED, "");
                self.param(ui, "Contrast", &mut conform.contrast, SIGNED, "");
                self.param(ui, "Soft Knee", &mut conform.soft_knee, PERCENT, "%");
            }
            _ => {}
        }
    }
}
