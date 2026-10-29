use timeline::{LightKind, LightingEffects, RadialShadow, ShadowRender};

use super::look::Rows;

const POINT: std::ops::RangeInclusive<f32> = -100.0..=200.0;
const SPREAD: std::ops::RangeInclusive<f32> = -100.0..=100.0;

impl Rows {
    pub(super) fn lighting_effects(&mut self, ui: &mut egui::Ui, effect: &mut LightingEffects) {
        for (number, light) in (1..).zip(&mut effect.lights) {
            self.options(
                ui,
                &format!("Light {number} Type"),
                &format!("light_kind_{number}"),
                &mut light.kind,
                LightKind::ALL,
                LightKind::name,
            );
            if light.kind == LightKind::None {
                continue;
            }
            self.color(ui, &format!("Light {number} Color"), &mut light.color);
            let [x, y] = &mut light.center;
            self.param(ui, &format!("Light {number} Center X"), x, POINT, "%");
            self.param(ui, &format!("Light {number} Center Y"), y, POINT, "%");
            let major = &mut light.major_radius;
            self.param(
                ui,
                &format!("Light {number} Major Radius"),
                major,
                0.0..=100.0,
                "%",
            );
            if light.kind == LightKind::Spotlight {
                let minor = &mut light.minor_radius;
                self.param(
                    ui,
                    &format!("Light {number} Minor Radius"),
                    minor,
                    0.0..=100.0,
                    "%",
                );
                let angle = &mut light.angle;
                self.param(
                    ui,
                    &format!("Light {number} Angle"),
                    angle,
                    -360.0..=360.0,
                    "°",
                );
            }
            let intensity = &mut light.intensity;
            self.param(
                ui,
                &format!("Light {number} Intensity"),
                intensity,
                0.0..=100.0,
                "",
            );
            if light.kind == LightKind::Spotlight {
                self.param(
                    ui,
                    &format!("Light {number} Focus"),
                    &mut light.focus,
                    SPREAD,
                    "",
                );
            }
        }
        self.color(ui, "Ambience Light Color", &mut effect.ambience_color);
        let ambience = &mut effect.ambience_intensity;
        self.param(ui, "Ambience Intensity", ambience, -100.0..=100.0, "");
        self.param(ui, "Surface Gloss", &mut effect.surface_gloss, SPREAD, "");
        self.param(
            ui,
            "Surface Material",
            &mut effect.surface_material,
            SPREAD,
            "",
        );
        self.param(ui, "Exposure", &mut effect.exposure, SPREAD, "");
        self.param(ui, "Bump Height", &mut effect.bump_height, 0.0..=100.0, "");
        let white_is_high = &mut effect.white_is_high;
        self.choice(ui, "White is High", |ui| {
            ui.checkbox(white_is_high, "").changed()
        });
    }

    pub(super) fn radial_shadow(&mut self, ui: &mut egui::Ui, shadow: &mut RadialShadow) {
        self.color(ui, "Shadow Color", &mut shadow.shadow_color);
        self.param(ui, "Opacity", &mut shadow.opacity, 0.0..=100.0, "%");
        let [x, y] = &mut shadow.light_source;
        self.param(ui, "Light Source X", x, POINT, "%");
        self.param(ui, "Light Source Y", y, POINT, "%");
        let distance = &mut shadow.projection_distance;
        self.param(ui, "Projection Distance", distance, 0.0..=200.0, "%");
        self.param(ui, "Softness", &mut shadow.softness, 0.0..=200.0, "px");
        self.options(
            ui,
            "Render",
            "radial_shadow_render",
            &mut shadow.render,
            ShadowRender::ALL,
            ShadowRender::name,
        );
        if shadow.render == ShadowRender::GlassEdge {
            let influence = &mut shadow.color_influence;
            self.param(ui, "Color Influence", influence, 0.0..=100.0, "%");
        }
        let only = &mut shadow.shadow_only;
        self.choice(ui, "Shadow Only", |ui| ui.checkbox(only, "").changed());
    }
}
