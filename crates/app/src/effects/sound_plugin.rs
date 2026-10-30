//! A VST3 plugin's parameters as rows, and the plugins to add.

use egui::RichText;
use timeline::{AudioEffect, Param, PluginEffect, PluginState};

use super::look::Rows;
use crate::vst::{Described, PluginHost};

/// Past this many parameters a filter field helps find one.
const FILTER_FROM: usize = 16;

pub(super) fn rows(
    ui: &mut egui::Ui,
    rows: &mut Rows,
    effect: &mut PluginEffect,
    host: &mut PluginHost,
) {
    let key = (effect.path.clone(), effect.class_id);
    let params = match host.describe(&key) {
        None => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Loading the plugin…");
            });
            return;
        }
        Some(Described::Failed(error)) => {
            let color = ui.visuals().warn_fg_color;
            ui.label(RichText::new(format!("Can't load this plugin: {error}")).color(color));
            return;
        }
        Some(Described::Params(params)) => params.clone(),
    };
    let filter_id = ui.make_persistent_id("plugin parameter filter");
    let mut filter: String = ui.data(|data| data.get_temp(filter_id)).unwrap_or_default();
    if params.len() > FILTER_FROM {
        let field = egui::TextEdit::singleline(&mut filter).hint_text("Find a parameter");
        if ui.add(field).changed() {
            ui.data_mut(|data| data.insert_temp(filter_id, filter.clone()));
        }
    }
    let filter = filter.to_lowercase();
    let listed = params
        .iter()
        .filter(|info| filter.is_empty() || info.title.to_lowercase().contains(&filter));
    for info in listed {
        let id = info.id.0;
        let default = info.default_normalized as f32;
        let mut param = effect
            .param(id)
            .cloned()
            .unwrap_or_else(|| Param::from_value(default));
        let before = param.clone();
        let shown = host
            .shown(&key, id, param.value_at(rows.time()))
            .map(|text| match info.units.as_str() {
                "" => text,
                units => format!("{text} {units}"),
            });
        ui.push_id(id, |ui| {
            rows.param_shown(ui, &info.title, &mut param, 0.0..=1.0, shown.as_deref());
        });
        if param != before {
            *effect.param_mut(id, default) = param;
        }
    }
}

/// The installed plugins to choose from, and a way to look for more.
pub(super) fn add_menu(ui: &mut egui::Ui, host: &mut PluginHost) -> Option<AudioEffect> {
    let mut chosen = None;
    if host.installed.is_empty() && !host.scanning {
        ui.label("No plugins found yet.");
    }
    for info in &host.installed {
        let button = ui.button(&info.name).on_hover_text(&info.vendor);
        if button.clicked() {
            chosen = Some(AudioEffect::Plugin(Box::new(PluginEffect {
                name: info.name.clone(),
                vendor: info.vendor.clone(),
                path: info.path.clone(),
                class_id: info.class_id.0,
                params: Vec::new(),
                state: PluginState::default(),
            })));
        }
    }
    ui.separator();
    if host.scanning {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Scanning…");
        });
    } else if ui
        .button("Scan for Plugins")
        .on_hover_text("Look through the standard VST3 folders")
        .clicked()
    {
        host.scan();
    }
    chosen
}
