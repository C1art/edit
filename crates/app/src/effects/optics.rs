use timeline::{LensType, MagnifyShape, VideoEffect};

use super::look::Rows;

const POINT: std::ops::RangeInclusive<f32> = -100.0..=200.0;

impl Rows {
    /// The controls of Lens Flare and Magnify; other effects have none here.
    pub(super) fn optics(&mut self, ui: &mut egui::Ui, effect: &mut VideoEffect) {
        match effect {
            VideoEffect::LensFlare {
                center: [x, y],
                brightness,
                lens,
                blend,
            } => {
                self.param(ui, "Flare Center X", x, POINT, "%");
                self.param(ui, "Flare Center Y", y, POINT, "%");
                self.param(ui, "Flare Brightness", brightness, 0.0..=300.0, "%");
                self.options(
                    ui,
                    "Lens Type",
                    "lens_type",
                    lens,
                    LensType::ALL,
                    LensType::name,
                );
                self.param(ui, "Blend With Original", blend, 0.0..=100.0, "%");
            }
            VideoEffect::Magnify {
                shape,
                center: [x, y],
                magnification,
                size,
                feather,
                opacity,
            } => {
                self.options(
                    ui,
                    "Shape",
                    "magnify_shape",
                    shape,
                    MagnifyShape::ALL,
                    MagnifyShape::name,
                );
                self.param(ui, "Center X", x, POINT, "%");
                self.param(ui, "Center Y", y, POINT, "%");
                self.param(ui, "Magnification", magnification, 100.0..=600.0, "%");
                self.param(ui, "Size", size, 0.0..=2000.0, "px");
                self.param(ui, "Feather", feather, 0.0..=500.0, "px");
                self.param(ui, "Opacity", opacity, 0.0..=100.0, "%");
            }
            _ => {}
        }
    }
}
