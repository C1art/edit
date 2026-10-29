use timeline::{PaintSurface, VideoEffect};

use super::look::Rows;

const POINT: std::ops::RangeInclusive<f32> = -100.0..=200.0;
const PERCENT: std::ops::RangeInclusive<f32> = 0.0..=100.0;
const SIGNED: std::ops::RangeInclusive<f32> = -100.0..=100.0;

impl Rows {
    /// The controls of Brush Strokes, Eyedropper Fill and SDR Conform; other effects
    /// have none here.
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
            VideoEffect::SdrConform(conform) => {
                self.param(ui, "Brightness", &mut conform.brightness, SIGNED, "");
                self.param(ui, "Contrast", &mut conform.contrast, SIGNED, "");
                self.param(ui, "Soft Knee", &mut conform.soft_knee, PERCENT, "%");
            }
            _ => {}
        }
    }
}
