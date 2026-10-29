use timeline::VideoEffect;

use super::look::Rows;

const POINT: std::ops::RangeInclusive<f32> = -100.0..=200.0;
const PIXELS: std::ops::RangeInclusive<f32> = 0.0..=2000.0;

impl Rows {
    /// The controls of the Generate category's drawn effects; other effects have none here.
    pub(super) fn generate(&mut self, ui: &mut egui::Ui, effect: &mut VideoEffect) {
        match effect {
            VideoEffect::Checkerboard {
                anchor: [x, y],
                size: [width, height],
                color,
                opacity,
            } => {
                self.param(ui, "Anchor X", x, POINT, "%");
                self.param(ui, "Anchor Y", y, POINT, "%");
                self.param(ui, "Width", width, 1.0..=2000.0, "px");
                self.param(ui, "Height", height, 1.0..=2000.0, "px");
                self.color(ui, "Color", color);
                self.param(ui, "Opacity", opacity, 0.0..=100.0, "%");
            }
            VideoEffect::Grid {
                anchor: [x, y],
                size: [width, height],
                border,
                color,
                opacity,
                invert,
            } => {
                self.param(ui, "Anchor X", x, POINT, "%");
                self.param(ui, "Anchor Y", y, POINT, "%");
                self.param(ui, "Width", width, 1.0..=2000.0, "px");
                self.param(ui, "Height", height, 1.0..=2000.0, "px");
                self.param(ui, "Border", border, 0.0..=200.0, "px");
                self.choice(ui, "Invert Grid", |ui| ui.checkbox(invert, "").changed());
                self.color(ui, "Color", color);
                self.param(ui, "Opacity", opacity, 0.0..=100.0, "%");
            }
            VideoEffect::Circle {
                center: [x, y],
                radius,
                feather,
                color,
                opacity,
                invert,
            } => {
                self.param(ui, "Center X", x, POINT, "%");
                self.param(ui, "Center Y", y, POINT, "%");
                self.param(ui, "Radius", radius, PIXELS, "px");
                self.param(ui, "Feather Outer Edge", feather, 0.0..=500.0, "px");
                self.choice(ui, "Invert Circle", |ui| ui.checkbox(invert, "").changed());
                self.color(ui, "Color", color);
                self.param(ui, "Opacity", opacity, 0.0..=100.0, "%");
            }
            VideoEffect::FourColorGradient {
                points,
                colors,
                blend,
                opacity,
            } => {
                for (number, ([x, y], color)) in (1..).zip(points.iter_mut().zip(colors)) {
                    self.param(ui, &format!("Point {number} X"), x, POINT, "%");
                    self.param(ui, &format!("Point {number} Y"), y, POINT, "%");
                    self.color(ui, &format!("Color {number}"), color);
                }
                self.param(ui, "Blend", blend, 1.0..=500.0, "");
                self.param(ui, "Opacity", opacity, 0.0..=100.0, "%");
            }
            _ => {}
        }
    }
}
