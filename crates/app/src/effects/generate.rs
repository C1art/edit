use timeline::{CellKind, VideoEffect};

use super::look::Rows;

const POINT: std::ops::RangeInclusive<f32> = -100.0..=200.0;
const PIXELS: std::ops::RangeInclusive<f32> = 0.0..=2000.0;

impl Rows {
    /// The controls of the Generate category's drawn effects; other effects have none here.
    pub(super) fn generate(&mut self, ui: &mut egui::Ui, effect: &mut VideoEffect) {
        match effect {
            VideoEffect::Checkerboard(timeline::Checkerboard {
                anchor: [x, y],
                size: [width, height],
                color,
                opacity,
            }) => {
                self.param(ui, "Anchor X", x, POINT, "%");
                self.param(ui, "Anchor Y", y, POINT, "%");
                self.param(ui, "Width", width, 1.0..=2000.0, "px");
                self.param(ui, "Height", height, 1.0..=2000.0, "px");
                self.color(ui, "Color", color);
                self.param(ui, "Opacity", opacity, 0.0..=100.0, "%");
            }
            VideoEffect::Grid(timeline::Grid {
                anchor: [x, y],
                size: [width, height],
                border,
                color,
                opacity,
                invert,
            }) => {
                self.param(ui, "Anchor X", x, POINT, "%");
                self.param(ui, "Anchor Y", y, POINT, "%");
                self.param(ui, "Width", width, 1.0..=2000.0, "px");
                self.param(ui, "Height", height, 1.0..=2000.0, "px");
                self.param(ui, "Border", border, 0.0..=200.0, "px");
                self.choice(ui, "Invert Grid", |ui| ui.checkbox(invert, "").changed());
                self.color(ui, "Color", color);
                self.param(ui, "Opacity", opacity, 0.0..=100.0, "%");
            }
            VideoEffect::Circle(timeline::Circle {
                center: [x, y],
                radius,
                feather,
                color,
                opacity,
                invert,
            }) => {
                self.param(ui, "Center X", x, POINT, "%");
                self.param(ui, "Center Y", y, POINT, "%");
                self.param(ui, "Radius", radius, PIXELS, "px");
                self.param(ui, "Feather Outer Edge", feather, 0.0..=500.0, "px");
                self.choice(ui, "Invert Circle", |ui| ui.checkbox(invert, "").changed());
                self.color(ui, "Color", color);
                self.param(ui, "Opacity", opacity, 0.0..=100.0, "%");
            }
            VideoEffect::FourColorGradient(timeline::FourColorGradient {
                points,
                colors,
                blend,
                opacity,
            }) => {
                for (number, ([x, y], color)) in (1..).zip(points.iter_mut().zip(colors)) {
                    self.param(ui, &format!("Point {number} X"), x, POINT, "%");
                    self.param(ui, &format!("Point {number} Y"), y, POINT, "%");
                    self.color(ui, &format!("Color {number}"), color);
                }
                self.param(ui, "Blend", blend, 1.0..=500.0, "");
                self.param(ui, "Opacity", opacity, 0.0..=100.0, "%");
            }
            VideoEffect::Ellipse(ellipse) => {
                let [x, y] = &mut ellipse.center;
                self.param(ui, "Center X", x, POINT, "%");
                self.param(ui, "Center Y", y, POINT, "%");
                self.param(ui, "Width", &mut ellipse.width, PIXELS, "px");
                self.param(ui, "Height", &mut ellipse.height, PIXELS, "px");
                self.param(ui, "Thickness", &mut ellipse.thickness, 0.0..=500.0, "px");
                self.param(ui, "Softness", &mut ellipse.softness, 0.0..=100.0, "%");
                self.color(ui, "Inside Color", &mut ellipse.inside_color);
                self.color(ui, "Outside Color", &mut ellipse.outside_color);
                let over = &mut ellipse.composite_on_original;
                self.choice(ui, "Composite On Original", |ui| {
                    ui.checkbox(over, "").changed()
                });
            }
            VideoEffect::CellPattern(cells) => {
                self.options(
                    ui,
                    "Cell Pattern",
                    "cell_pattern_kind",
                    &mut cells.pattern,
                    CellKind::ALL,
                    CellKind::name,
                );
                let invert = &mut cells.invert;
                self.choice(ui, "Invert", |ui| ui.checkbox(invert, "").changed());
                self.param(ui, "Contrast", &mut cells.contrast, 0.0..=400.0, "%");
                self.param(ui, "Disperse", &mut cells.disperse, 0.0..=150.0, "%");
                self.param(ui, "Size", &mut cells.size, 2.0..=500.0, "px");
                let [x, y] = &mut cells.offset;
                self.param(ui, "Offset X", x, -2000.0..=2000.0, "px");
                self.param(ui, "Offset Y", y, -2000.0..=2000.0, "px");
                self.param(ui, "Evolution", &mut cells.evolution, -3600.0..=3600.0, "°");
                let blend = &mut cells.blend_with_original;
                self.param(ui, "Blend With Original", blend, 0.0..=100.0, "%");
            }
            VideoEffect::Lightning(bolt) => {
                for (label, [x, y]) in [
                    ("Start Point", &mut bolt.start_point),
                    ("End Point", &mut bolt.end_point),
                ] {
                    self.param(ui, &format!("{label} X"), x, POINT, "%");
                    self.param(ui, &format!("{label} Y"), y, POINT, "%");
                }
                self.param(ui, "Segments", &mut bolt.segments, 1.0..=100.0, "");
                self.param(ui, "Amplitude", &mut bolt.amplitude, 0.0..=100.0, "%");
                self.param(ui, "Branching", &mut bolt.branching, 0.0..=100.0, "%");
                self.param(ui, "Width", &mut bolt.width, 1.0..=100.0, "px");
                self.param(ui, "Core Width", &mut bolt.core_width, 0.0..=100.0, "%");
                self.color(ui, "Outside Color", &mut bolt.outside_color);
                self.color(ui, "Inside Color", &mut bolt.inside_color);
                self.param(ui, "Speed", &mut bolt.speed, 0.0..=60.0, "/s");
                let seed = &mut bolt.random_seed;
                self.choice(ui, "Random Seed", |ui| {
                    ui.add(egui::DragValue::new(seed).update_while_editing(false))
                        .changed()
                });
            }
            _ => {}
        }
    }
}
