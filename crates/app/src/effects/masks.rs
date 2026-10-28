use egui::RichText;
use render::MAX_MASKS;
use timeline::{Mask, MaskShape};

use super::look::Rows;

/// The masks under Opacity: each shape's geometry and edge, and buttons to add more.
pub(super) fn controls(ui: &mut egui::Ui, rows: &mut Rows, masks: &mut Vec<Mask>) {
    let mut removed = None;
    for (index, mask) in masks.iter_mut().enumerate() {
        ui.push_id(("mask", index), |ui| {
            ui.horizontal(|ui| {
                let title = format!("Mask ({}) – {}", index + 1, mask.shape.name());
                ui.label(RichText::new(title).strong());
                if ui.small_button("Remove").clicked() {
                    removed = Some(index);
                }
            });
            let [x, y] = &mut mask.center;
            rows.param(ui, "Center X", x, -100.0..=200.0, "%");
            rows.param(ui, "Center Y", y, -100.0..=200.0, "%");
            let [width, height] = &mut mask.size;
            rows.param(ui, "Width", width, 0.0..=400.0, "%");
            rows.param(ui, "Height", height, 0.0..=400.0, "%");
            rows.param(ui, "Rotation", &mut mask.rotation, -3_600.0..=3_600.0, "°");
            rows.param(ui, "Feather", &mut mask.feather, 0.0..=1_000.0, " px");
            rows.param(ui, "Opacity", &mut mask.opacity, 0.0..=100.0, "%");
            rows.param(
                ui,
                "Expansion",
                &mut mask.expansion,
                -1_000.0..=1_000.0,
                " px",
            );
            if ui.checkbox(&mut mask.inverted, "Inverted").changed() {
                rows.commit();
            }
        });
    }
    if let Some(index) = removed {
        masks.remove(index);
        rows.commit();
    }
    ui.horizontal(|ui| {
        for shape in MaskShape::ALL {
            let add = egui::Button::new(format!("Add {} Mask", shape.name())).small();
            if ui.add_enabled(masks.len() < MAX_MASKS, add).clicked() {
                masks.push(Mask::new(shape));
                rows.commit();
            }
        }
    });
}
