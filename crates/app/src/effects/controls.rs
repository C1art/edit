use timeline::Rgb;

use super::look::Rows;

impl Rows {
    /// A labelled row of controls that return whether they made a choice.
    pub(super) fn choice(
        &mut self,
        ui: &mut egui::Ui,
        label: &str,
        add: impl FnOnce(&mut egui::Ui) -> bool,
    ) {
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            ui.add_sized([72.0, 18.0], egui::Label::new(label));
            if add(ui) {
                self.commit();
            }
        });
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

    pub(super) fn color(&mut self, ui: &mut egui::Ui, label: &str, color: &mut Rgb) {
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            ui.add_sized([72.0, 18.0], egui::Label::new(label));
            let mut rgb = [color.r, color.g, color.b];
            if ui.color_edit_button_srgb(&mut rgb).changed() {
                *color = Rgb::new(rgb[0], rgb[1], rgb[2]);
                self.commit();
            }
        });
    }
}
