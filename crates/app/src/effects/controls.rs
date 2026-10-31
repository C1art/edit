use std::ops::RangeInclusive;

use egui::DragValue;
use timeline::Rgb;

use super::look::Rows;
use crate::mixer::response_state;

const LABEL_INDENT: f32 = 20.0;
const LABEL_WIDTH: f32 = 72.0;

/// A row's name, where every row's name goes so their values line up.
pub(super) fn label_column(ui: &mut egui::Ui, label: &str) {
    ui.add_space(LABEL_INDENT);
    ui.add_sized([LABEL_WIDTH, 18.0], egui::Label::new(label));
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
        let text_width = egui::WidgetText::from(text)
            .into_galley(
                ui,
                Some(egui::TextWrapMode::Extend),
                f32::INFINITY,
                egui::TextStyle::Button,
            )
            .size()
            .x;
        let spacing = ui.spacing();
        let width = LABEL_INDENT
            + LABEL_WIDTH
            + spacing.item_spacing.x * 2.0
            + spacing.icon_width
            + spacing.icon_spacing
            + text_width;
        let changed = ui
            .horizontal(|ui| {
                // A narrow panel can't fit the box in the value column, so it moves to the label column.
                if ui.available_width() >= width {
                    label_column(ui, "");
                } else {
                    ui.add_space(LABEL_INDENT);
                }
                ui.checkbox(value, text).changed()
            })
            .inner;
        if changed {
            self.commit();
        }
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
            let response = ui.add(
                DragValue::new(value)
                    .update_while_editing(false)
                    .range(range)
                    .suffix(suffix),
            );
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
