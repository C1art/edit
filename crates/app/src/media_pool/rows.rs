use egui::{Key, Label, RichText, Sense, TextEdit};
use timeline::{FrameRate, MediaItem, TimeDisplay, Timebase, Timecode};

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
    offline: bool,
) -> egui::Response {
    let mut details = vec![match &item.generator {
        Some(generator) => generator.kind_name().to_owned(),
        None => played_length(item, timebase).to_string(),
    }];
    if item.nest.is_some() {
        details.insert(0, "Sequence".to_owned());
    }
    if item.subclip.is_some() {
        details.insert(0, "Subclip".to_owned());
    }
    if item.still {
        details.insert(0, "Still".to_owned());
    }
    if let (Some(stored), Some(video)) = (item.video, item.played_format()) {
        let interpreted = if item.is_interpreted() {
            " (interpreted)"
        } else {
            ""
        };
        let aspect = item.pixel_aspect();
        let pixels = if aspect.is_square() {
            String::new()
        } else {
            format!(" ({:.4})", aspect.ratio())
        };
        details.push(format!(
            "{}×{}{pixels} {:.2} fps{interpreted}",
            stored.width,
            stored.height,
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
        let name = RichText::new(&item.name).strong();
        if offline {
            let name = name.color(ui.visuals().error_fg_color);
            ui.add(Label::new(name).truncate());
            ui.add(Label::new(RichText::new("Media offline").small()).truncate());
        } else {
            ui.add(Label::new(name).truncate());
        }
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
            (None, None) if offline => format!("Missing: {}", item.path.display()),
            (None, None) => item.path.display().to_string(),
        })
}

/// How long the item's default range plays for, at its interpreted rate.
pub(super) fn played_length(item: &MediaItem, timebase: Timebase) -> Timecode {
    let rate = item
        .played_format()
        .map_or(FrameRate::FPS_25, |video| video.frame_rate);
    let range = item.default_range(timebase);
    let played = item.conform().sequence_span(range.end - range.start);
    Timecode::shown(played, timebase, rate, TimeDisplay::Timecode)
}
