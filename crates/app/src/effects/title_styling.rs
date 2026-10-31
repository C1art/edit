use timeline::{FillGradient, GradientKind, ShadowStyle, TextStroke};

use super::generated::{color_button, pairs};
use crate::mixer::{response_state, Change};

pub(super) fn stroke_row(ui: &mut egui::Ui, stroke: &mut Option<TextStroke>) -> Option<Change> {
    let mut change = None;
    ui.horizontal_wrapped(|ui| {
        let mut stroked = stroke.is_some();
        if ui.checkbox(&mut stroked, "Stroke").changed() {
            *stroke = stroked.then(TextStroke::default);
            change = Some(Change::Commit);
        }
        if let Some(stroke) = stroke {
            change = color_button(ui, &mut stroke.color).or(change);
            let width = ui.add(
                egui::DragValue::new(&mut stroke.width)
                    .update_while_editing(false)
                    .range(1..=100)
                    .suffix(" px"),
            );
            change = response_state(&width, change);
        }
    });
    change
}

pub(super) fn gradient_row(
    ui: &mut egui::Ui,
    gradient: &mut Option<FillGradient>,
) -> Option<Change> {
    let mut change = None;
    ui.horizontal_wrapped(|ui| {
        let mut blended = gradient.is_some();
        if ui.checkbox(&mut blended, "Gradient").changed() {
            *gradient = blended.then(FillGradient::default);
            change = Some(Change::Commit);
        }
        let Some(gradient) = gradient else {
            return;
        };
        change = color_button(ui, &mut gradient.to).or(change);
        for kind in GradientKind::ALL {
            if ui
                .selectable_value(&mut gradient.kind, kind, kind.name())
                .changed()
            {
                change = Some(Change::Commit);
            }
        }
        if gradient.kind == GradientKind::Linear {
            let angle = egui::DragValue::new(&mut gradient.angle)
                .update_while_editing(false)
                .range(-360..=360)
                .suffix("°");
            change = response_state(&ui.add(angle).on_hover_text("Angle"), change);
        }
    });
    change
}

/// Essential Graphics' shadow: until one of its controls is changed, the `usual` look
/// that scales with what casts it.
pub(super) fn shadow_rows(
    ui: &mut egui::Ui,
    shadow: &mut bool,
    style: &mut Option<ShadowStyle>,
    usual: ShadowStyle,
) -> Option<Change> {
    let mut change = None;
    let mut edited = style.unwrap_or(usual);
    ui.horizontal_wrapped(|ui| {
        if ui.checkbox(shadow, "Drop shadow").changed() {
            change = Some(Change::Commit);
        }
        if *shadow {
            change = color_button(ui, &mut edited.color).or(change);
            let opacity = egui::DragValue::new(&mut edited.opacity)
                .update_while_editing(false)
                .range(0..=100)
                .suffix("%");
            change = response_state(&ui.add(opacity).on_hover_text("Opacity"), change);
            let angle = egui::DragValue::new(&mut edited.angle)
                .update_while_editing(false)
                .range(-360..=360)
                .suffix("°");
            change = response_state(&ui.add(angle).on_hover_text("Angle"), change);
        }
    });
    if *shadow {
        pairs(ui, 255.0, |ui| {
            for (value, name) in [
                (&mut edited.distance, "Distance"),
                (&mut edited.size, "Size"),
                (&mut edited.blur, "Blur"),
            ] {
                ui.horizontal(|ui| {
                    ui.label(name);
                    let drag = egui::DragValue::new(value)
                        .update_while_editing(false)
                        .range(0..=200)
                        .suffix(" px");
                    change = response_state(&ui.add(drag), change);
                });
            }
        });
    }
    if edited != style.unwrap_or(usual) {
        *style = Some(edited);
    }
    change
}
