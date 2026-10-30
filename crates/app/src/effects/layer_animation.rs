use egui::ComboBox;
use timeline::{Appear, GraphicContent, GraphicLayer, Pin, VerticalPin};

use super::generated::pairs;
use crate::mixer::{response_state, Change};

/// Which edge of a text or shape layer holds still as it grows, and how the layer comes
/// on and goes off.
pub(super) fn animation_controls(ui: &mut egui::Ui, layer: &mut GraphicLayer) -> Option<Change> {
    let mut change = None;
    if !matches!(layer.content, GraphicContent::Image(_)) {
        ui.horizontal(|ui| {
            ui.label("Pin");
            for pin in Pin::ALL {
                if ui
                    .selectable_label(layer.pin == pin, pin.name())
                    .on_hover_text("The edge that stays put as the text or shape grows")
                    .clicked()
                    && layer.pin != pin
                {
                    layer.pin = pin;
                    change = Some(Change::Commit);
                }
            }
            ui.separator();
            for pin in VerticalPin::ALL {
                if ui
                    .selectable_label(layer.pin_vertical == pin, pin.name())
                    .on_hover_text("The edge that stays put as the text or shape grows taller")
                    .clicked()
                    && layer.pin_vertical != pin
                {
                    layer.pin_vertical = pin;
                    change = Some(Change::Commit);
                }
            }
        });
    }
    let animation = &mut layer.animation;
    pairs(ui, 300.0, |ui| {
        for (label, appear) in [("In", &mut animation.enter), ("Out", &mut animation.exit)] {
            ui.horizontal(|ui| {
                ui.label(label);
                ComboBox::from_id_salt(("layer appear", label))
                    .selected_text(appear.name())
                    .width(110.0)
                    .show_ui(ui, |ui| {
                        for choice in Appear::all() {
                            if ui.selectable_value(appear, choice, choice.name()).clicked() {
                                change = Some(Change::Commit);
                            }
                        }
                    });
            });
        }
    });
    if animation.is_still() {
        return change;
    }
    pairs(ui, 300.0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Delay");
            let delay = egui::DragValue::new(&mut animation.delay)
                .range(0..=600)
                .suffix(" fr");
            let delay = ui
                .add(delay)
                .on_hover_text("Frames after the clip starts, and before it ends");
            change = response_state(&delay, change);
        });
        ui.horizontal(|ui| {
            ui.label("Length");
            let length = egui::DragValue::new(&mut animation.length)
                .range(1..=600)
                .suffix(" fr");
            change = response_state(&ui.add(length), change);
        });
    });
    change
}
