use egui::{Rect, RichText, UiBuilder};
use timeline::{Sequence, TrackKind, TrackRef, TrackSwitch};

use super::layout::{Row, ROW_HEIGHT};
use super::Targets;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderAction {
    ToggleTarget(TrackRef),
    Switch(TrackRef, TrackSwitch, bool),
}

/// The track name, which doubles as the source-patching target toggle, and the mute,
/// lock and, for audio, solo switches.
pub fn header(
    ui: &mut egui::Ui,
    column: egui::Rangef,
    row: &Row,
    sequence: &Sequence,
    targets: &Targets,
) -> Option<HeaderAction> {
    let track = sequence.track(row.track)?;
    let rect = Rect::from_x_y_ranges(column, row.y_range()).shrink2(egui::vec2(4.0, 0.0));
    let mut action = None;
    ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
        ui.set_clip_rect(rect.intersect(ui.clip_rect()));
        ui.horizontal_centered(|ui| {
            ui.set_min_height(ROW_HEIGHT);
            let targeted = targets.contains(row.track);
            let name = ui
                .selectable_label(targeted, RichText::new(row.track.to_string()).strong())
                .on_hover_text("Target this track for Insert and Overwrite");
            if name.clicked() {
                action = Some(HeaderAction::ToggleTarget(row.track));
            }
            let mute_label = match row.track.kind {
                TrackKind::Video => "Hide",
                TrackKind::Audio => "Mute",
            };
            let solo = (row.track.kind == TrackKind::Audio).then_some((
                "Solo",
                TrackSwitch::Solo,
                track.solo,
            ));
            let switches = [
                (mute_label, TrackSwitch::Mute, track.muted),
                ("Lock", TrackSwitch::Lock, track.locked),
            ];
            for (label, switch, on) in switches.into_iter().chain(solo) {
                if ui.selectable_label(on, label).clicked() {
                    action = Some(HeaderAction::Switch(row.track, switch, !on));
                }
            }
        });
    });
    action
}
