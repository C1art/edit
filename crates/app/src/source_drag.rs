use egui::{CursorIcon, Response, Sense};

use crate::icons::{self, Icon};
use crate::source::SourceMonitor;
use crate::timeline_view::{SourceDrag, Take};

const HANDLE: egui::Vec2 = egui::vec2(18.0, 16.0);

/// Lets the picture carry the marked part of its media out to the timeline.
pub fn picture(ui: &egui::Ui, source: &SourceMonitor, picture: &Response) -> Response {
    let response = ui.interact(
        picture.rect,
        picture.id.with("drag"),
        Sense::click_and_drag(),
    );
    offer(ui, &response, source, Take::Both);
    response
}

/// Premiere's handles for dragging out only the picture or only the sound of media
/// that has both.
pub fn handles(ui: &mut egui::Ui, source: &SourceMonitor, has_both: bool) {
    if !has_both {
        return;
    }
    let handles = [
        (Icon::Waveform, Take::AudioOnly, "Drag Audio Only"),
        (Icon::FilmStrip, Take::VideoOnly, "Drag Video Only"),
    ];
    for (icon, take, tip) in handles {
        let response = icons::button(ui, icon, HANDLE)
            .interact(Sense::drag())
            .on_hover_cursor(CursorIcon::Grab)
            .on_hover_text(tip);
        offer(ui, &response, source, take);
    }
}

fn offer(ui: &egui::Ui, response: &Response, source: &SourceMonitor, take: Take) {
    if response.drag_started() {
        response.dnd_set_drag_payload(SourceDrag {
            source: source.source,
            range: source.range(),
            take,
        });
    }
    if response.dragged() {
        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
    }
}
