use egui::{Label, RichText, Sense};
use timeline::{FrameRate, MediaItem, SourceId, Timebase, Timecode};

use crate::proxies::ProxyMaker;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolAction {
    Open(SourceId),
    /// Opens a nested sequence in the timeline.
    OpenNest(SourceId),
    Remove(SourceId),
    CreateProxy(SourceId),
    DetachProxy(SourceId),
}

/// Lists imported media. Click opens an item in the source monitor; dragging one onto
/// the timeline places it.
pub fn ui(
    ui: &mut egui::Ui,
    media: &[MediaItem],
    timebase: Timebase,
    importing: bool,
    proxies: Option<&ProxyMaker>,
) -> Option<PoolAction> {
    let mut action = None;
    if media.is_empty() && !importing {
        ui.label("Drop files here or use File → Import (Ctrl+I).");
    }
    egui::ScrollArea::vertical()
        .auto_shrink([false, true])
        .max_height(ui.available_height() * 0.6)
        .show(ui, |ui| {
            for item in media {
                let building = proxies.and_then(|maker| maker.progress(item.id));
                let response = item_row(ui, item, timebase, building);
                if response.drag_started() {
                    response.dnd_set_drag_payload(item.id);
                }
                if response.clicked() {
                    action = Some(PoolAction::Open(item.id));
                }
                if response.double_clicked() && item.nest.is_some() {
                    action = Some(PoolAction::OpenNest(item.id));
                }
                response.context_menu(|ui| {
                    if ui.button("Open in Source Monitor").clicked() {
                        action = Some(PoolAction::Open(item.id));
                    }
                    if item.nest.is_some() && ui.button("Open in Timeline").clicked() {
                        action = Some(PoolAction::OpenNest(item.id));
                    }
                    if ui.button("Remove from Project").clicked() {
                        action = Some(PoolAction::Remove(item.id));
                    }
                    if item.video.is_some() && item.is_file() {
                        ui.separator();
                        let label = if item.proxy.is_some() {
                            "Recreate Proxy"
                        } else {
                            "Create Proxy"
                        };
                        let enabled = proxies.is_some() && building.is_none();
                        if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                            action = Some(PoolAction::CreateProxy(item.id));
                        }
                        let detach = egui::Button::new("Detach Proxy");
                        if ui.add_enabled(item.proxy.is_some(), detach).clicked() {
                            action = Some(PoolAction::DetachProxy(item.id));
                        }
                    }
                });
            }
        });
    if importing {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Importing…");
        });
    }
    action
}

fn item_row(
    ui: &mut egui::Ui,
    item: &MediaItem,
    timebase: Timebase,
    building: Option<f32>,
) -> egui::Response {
    let rate = item
        .video
        .map_or(FrameRate::FPS_25, |video| video.frame_rate);
    let mut details = vec![match &item.generator {
        Some(generator) => generator.kind_name().to_owned(),
        None => Timecode::new(item.duration, timebase, rate).to_string(),
    }];
    if item.nest.is_some() {
        details.insert(0, "Sequence".to_owned());
    }
    if let Some(video) = item.video {
        details.push(format!(
            "{}×{} {:.2} fps",
            video.width,
            video.height,
            video.frame_rate.approx_fps()
        ));
    }
    if item.has_audio {
        details.push("audio".to_owned());
    }
    if item.proxy.is_some() {
        details.push("proxy".to_owned());
    }
    let inner = ui.group(|ui| {
        ui.set_width(ui.available_width());
        ui.add(Label::new(RichText::new(&item.name).strong()).truncate());
        ui.add(Label::new(RichText::new(details.join(" · ")).small()).truncate());
        if let Some(done) = building {
            let bar = egui::ProgressBar::new(done).text(RichText::new("Creating proxy").small());
            ui.add(bar.desired_height(14.0));
        }
    });
    inner
        .response
        .interact(Sense::click_and_drag())
        .on_hover_text(match (&item.generator, &item.nest) {
            (Some(generator), _) => generator.kind_name().to_owned(),
            (None, Some(_)) => "Nested sequence; double-click to open".to_owned(),
            (None, None) => item.path.display().to_string(),
        })
}
