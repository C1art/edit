use timeline::{Bevel, RoughenEdgeType, VideoEffect};

use super::look::Rows;

const ANGLE: std::ops::RangeInclusive<f32> = -360.0..=360.0;

impl Rows {
    /// The controls of the edge effects; other effects have none here.
    pub(super) fn edges(&mut self, ui: &mut egui::Ui, effect: &mut VideoEffect) {
        match effect {
            VideoEffect::RoughenEdges(rough) => {
                self.options(
                    ui,
                    "Edge Type",
                    "roughen_type",
                    &mut rough.edge_type,
                    RoughenEdgeType::ALL,
                    RoughenEdgeType::name,
                );
                if rough.edge_type == RoughenEdgeType::RoughenColor {
                    self.color(ui, "Edge Color", &mut rough.edge_color);
                }
                self.param(ui, "Border", &mut rough.border, 0.0..=500.0, "px");
                self.param(
                    ui,
                    "Edge Sharpness",
                    &mut rough.edge_sharpness,
                    0.0..=10.0,
                    "",
                );
                let influence = &mut rough.fractal_influence;
                self.param(ui, "Fractal Influence", influence, 0.0..=1.0, "");
                self.param(ui, "Scale", &mut rough.scale, 10.0..=1000.0, "%");
                let [x, y] = &mut rough.offset;
                self.param(ui, "Offset X", x, -10_000.0..=10_000.0, "px");
                self.param(ui, "Offset Y", y, -10_000.0..=10_000.0, "px");
                self.param(ui, "Complexity", &mut rough.complexity, 1.0..=10.0, "");
                self.param(ui, "Evolution", &mut rough.evolution, -3600.0..=3600.0, "°");
            }
            VideoEffect::BevelEdges(bevel) => self.bevel(ui, bevel, 0.0..=50.0, "%"),
            VideoEffect::BevelAlpha(bevel) => self.bevel(ui, bevel, 0.0..=200.0, "px"),
            VideoEffect::AlphaGlow(glow) => {
                self.param(ui, "Glow", &mut glow.glow, 0.0..=100.0, "px");
                self.param(ui, "Brightness", &mut glow.brightness, 0.0..=255.0, "");
                self.color(ui, "Start Color", &mut glow.start_color);
                let mut use_end = glow.end_color.is_some();
                self.choice(ui, "Use End Color", |ui| {
                    ui.checkbox(&mut use_end, "").changed()
                });
                match (use_end, &mut glow.end_color) {
                    (true, Some(end)) => self.color(ui, "End Color", end),
                    (true, end) => *end = Some(glow.start_color),
                    (false, end) => *end = None,
                }
                let fade = &mut glow.fade_out;
                self.choice(ui, "Fade Out", |ui| ui.checkbox(fade, "").changed());
            }
            VideoEffect::ColorEmboss {
                direction,
                relief,
                contrast,
                blend,
            } => {
                self.param(ui, "Direction", direction, ANGLE, "°");
                self.param(ui, "Relief", relief, 1.0..=10.0, "");
                self.param(ui, "Contrast", contrast, 0.0..=500.0, "%");
                self.param(ui, "Blend With Original", blend, 0.0..=100.0, "%");
            }
            _ => {}
        }
    }

    fn bevel(
        &mut self,
        ui: &mut egui::Ui,
        bevel: &mut Bevel,
        thickness: std::ops::RangeInclusive<f32>,
        suffix: &str,
    ) {
        self.param(
            ui,
            "Edge Thickness",
            &mut bevel.edge_thickness,
            thickness,
            suffix,
        );
        self.param(ui, "Light Angle", &mut bevel.light_angle, ANGLE, "°");
        self.color(ui, "Light Color", &mut bevel.light_color);
        let intensity = &mut bevel.light_intensity;
        self.param(ui, "Light Intensity", intensity, 0.0..=100.0, "%");
    }
}
