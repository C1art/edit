use std::ops::RangeInclusive;

use egui::DragValue;
use timeline::Rgb;

use super::look::Rows;
use crate::mixer::response_state;

/// A row's name, where every row's name goes so their values line up.
pub(super) fn label_column(ui: &mut egui::Ui, label: &str) {
    ui.add_space(20.0);
    ui.add_sized([72.0, 18.0], egui::Label::new(label));
}

impl Rows {
    /// A labelled row of controls that return whether they made a choice.
    pub(super) fn choice(
        &mut self,
        ui: &mut egui::Ui,
        label: &str,
        add: impl FnOnce(&mut egui::Ui) -> bool,
    ) {
        ui.horizontal(|ui| {
            label_column(ui, label);
            if add(ui) {
                self.commit();
            }
        });
    }

    /// A checkbox lined up with the values, returning whether it was ticked or cleared.
    pub(super) fn checkbox(&mut self, ui: &mut egui::Ui, value: &mut bool, text: &str) -> bool {
        let mut changed = false;
        self.choice(ui, "", |ui| {
            changed = ui.checkbox(value, text).changed();
            changed
        });
        changed
    }

    /// A drop-down of `all`, named by `name`.
    pub(super) fn options<T: Copy + PartialEq>(
        &mut self,
        ui: &mut egui::Ui,
        label: &str,
        id: &str,
        value: &mut T,
        all: impl IntoIterator<Item = T>,
        name: fn(T) -> &'static str,
    ) {
        self.choice(ui, label, |ui| {
            let mut chosen = false;
            egui::ComboBox::from_id_salt(id)
                .selected_text(name(*value))
                .show_ui(ui, |ui| {
                    for option in all {
                        chosen |= ui.selectable_value(value, option, name(option)).clicked();
                    }
                });
            chosen
        });
    }

    /// A setting in whole numbers that can't be keyframed.
    pub(super) fn whole(
        &mut self,
        ui: &mut egui::Ui,
        label: &str,
        value: &mut u32,
        range: RangeInclusive<u32>,
        suffix: &str,
    ) {
        ui.horizontal(|ui| {
            label_column(ui, label);
            let response = ui.add(DragValue::new(value).range(range).suffix(suffix));
            self.output.change = response_state(&response, self.output.change);
        });
    }

    pub(super) fn color(&mut self, ui: &mut egui::Ui, label: &str, color: &mut Rgb) {
        ui.horizontal(|ui| {
            label_column(ui, label);
            let mut rgb = [color.r, color.g, color.b];
            if ui.color_edit_button_srgb(&mut rgb).changed() {
                *color = Rgb::new(rgb[0], rgb[1], rgb[2]);
                self.commit();
            }
        });
    }
}
