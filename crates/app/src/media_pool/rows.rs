use egui::{Key, Label, RichText, Sense, TextEdit};
use timeline::{FrameRate, MediaItem, Timebase, Timecode};

use super::PoolAction;

pub(super) enum Field {
    Editing,
    Done,
    Cancelled,
}

pub(super) fn proxy_menu(
    ui: &mut egui::Ui,
    item: &MediaItem,
    can_build: bool,
    action: &mut Option<PoolAction>,
) {
    let label = if item.proxy.is_some() {
        "Recreate Proxy"
    } else {
        "Create Proxy"
    };
    if ui
        .add_enabled(can_build, egui::Button::new(label))
        .clicked()
    {
        *action = Some(PoolAction::CreateProxy(item.id));
    }
    let detach = egui::Button::new("Detach Proxy");
    if ui.add_enabled(item.proxy.is_some(), detach).clicked() {
        *action = Some(PoolAction::DetachProxy(item.id));
    }
}

/// A field taking an item's new name: Enter or clicking away keeps it, Escape drops it.
pub(super) fn rename_field(ui: &mut egui::Ui, name: &mut String) -> Field {
    let response = ui.add(TextEdit::singleline(name).desired_width(f32::INFINITY));
    if response.lost_focus() {
        if ui.input(|input| input.key_pressed(Key::Escape)) {
            Field::Cancelled
        } else {
            Field::Done
        }
    } else {
        if !response.has_focus() {
            response.request_focus();
        }
        Field::Editing
    }
}

pub(super) fn item_row(
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
