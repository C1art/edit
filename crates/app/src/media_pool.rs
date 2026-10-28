use egui::{Label, RichText, Sense};
use timeline::{FrameRate, MediaItem, SourceId, Timebase, Timecode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolAction {
    Open(SourceId),
    Remove(SourceId),
}

/// Lists imported media. Click opens an item in the source monitor; dragging one onto
/// the timeline places it.
pub fn ui(
    ui: &mut egui::Ui,
    media: &[MediaItem],
    timebase: Timebase,
    importing: bool,
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
                let response = item_row(ui, item, timebase);
                if response.drag_started() {
                    response.dnd_set_drag_payload(item.id);
                }
                if response.clicked() {
                    action = Some(PoolAction::Open(item.id));
                }
                response.context_menu(|ui| {
                    if ui.button("Open in Source Monitor").clicked() {
                        action = Some(PoolAction::Open(item.id));
                    }
                    if ui.button("Remove from Project").clicked() {
                        action = Some(PoolAction::Remove(item.id));
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

fn item_row(ui: &mut egui::Ui, item: &MediaItem, timebase: Timebase) -> egui::Response {
    let rate = item
        .video
        .map_or(FrameRate::FPS_25, |video| video.frame_rate);
    let mut details = vec![Timecode::new(item.duration, timebase, rate).to_string()];
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
    let inner = ui.group(|ui| {
        ui.set_width(ui.available_width());
        ui.add(Label::new(RichText::new(&item.name).strong()).truncate());
        ui.add(Label::new(RichText::new(details.join(" · ")).small()).truncate());
    });
    inner
        .response
        .interact(Sense::click_and_drag())
        .on_hover_text(item.path.display().to_string())
}
