//! Text styles in a title's controls: picking one gives the title its look, and the
//! title's look can be kept as a new one.

use std::sync::Arc;

use timeline::{TextStyle, Title};

use crate::mixer::Change;

/// What the style row asks of the editor, which keeps the styles.
#[derive(Clone)]
pub(crate) enum StyleRequest {
    Save(Box<Title>),
    Delete(String),
}

/// Lends the kept styles to this frame's controls.
pub(super) fn lend(ctx: &egui::Context, styles: &Arc<Vec<TextStyle>>) {
    ctx.data_mut(|data| data.insert_temp(styles_id(), Arc::clone(styles)));
}

pub(super) fn take_request(ctx: &egui::Context) -> Option<StyleRequest> {
    ctx.data_mut(|data| data.remove_temp::<Option<StyleRequest>>(request_id()))
        .flatten()
}

pub(super) fn row(ui: &mut egui::Ui, title: &mut Title) -> Option<Change> {
    let styles: Arc<Vec<TextStyle>> = ui
        .data(|data| data.get_temp(styles_id()))
        .unwrap_or_default();
    let mut change = None;
    let mut request = None;
    ui.horizontal_wrapped(|ui| {
        ui.label("Style");
        let current = styles
            .iter()
            .find(|style| style.is_on(title))
            .map_or("None", |style| style.name.as_str());
        egui::ComboBox::from_id_salt("text_style")
            .selected_text(current)
            .show_ui(ui, |ui| {
                if styles.is_empty() {
                    ui.weak("Keep a look with Save Style…");
                }
                for style in styles.iter() {
                    let response = ui.selectable_label(style.name == current, &style.name);
                    if response.clicked() {
                        style.apply(title);
                        change = Some(Change::Commit);
                    }
                    response.context_menu(|ui| {
                        if ui.button("Delete Style").clicked() {
                            request = Some(StyleRequest::Delete(style.name.clone()));
                            ui.close();
                        }
                    });
                }
            });
        if ui
            .button("Save Style…")
            .on_hover_text("Keep this look to give other titles")
            .clicked()
        {
            request = Some(StyleRequest::Save(Box::new(title.clone())));
        }
    });
    if request.is_some() {
        ui.data_mut(|data| data.insert_temp(request_id(), request));
    }
    change
}

fn styles_id() -> egui::Id {
    egui::Id::unique("text_styles_lent")
}

fn request_id() -> egui::Id {
    egui::Id::unique("text_style_request")
}
