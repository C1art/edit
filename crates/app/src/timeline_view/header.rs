use egui::{Label, Rect, RichText, Sense, TextEdit, UiBuilder};
use timeline::{Sequence, Track, TrackKind, TrackRef, TrackSwitch};

use super::layout::Row;
use super::Targets;
use crate::icons::{self, Icon, Look};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeaderAction {
    ToggleTarget(TrackRef),
    /// Shows the track's volume automation in place of its clips' volume lines, or back.
    ToggleAutomation(TrackRef),
    Switch(TrackRef, TrackSwitch, bool),
    Rename(TrackRef, String),
    Add(TrackKind),
    Delete(TrackRef),
}

/// Tall enough for the switches and the name on lines of their own.
const TWO_LINE_HEIGHT: f32 = 44.0;

/// A track name being typed in its header.
#[derive(Debug, Clone)]
pub struct Renaming {
    track: TrackRef,
    text: String,
    focused: bool,
}

/// The track name, which doubles as the source-patching target toggle, and the mute,
/// lock and, for audio, solo switches and the automation toggle.
pub fn header(
    ui: &mut egui::Ui,
    column: egui::Rangef,
    row: &Row,
    sequence: &Sequence,
    targets: &Targets,
    automation: bool,
    renaming: &mut Option<Renaming>,
) -> Option<HeaderAction> {
    let track = sequence.track(row.track)?;
    let rect = Rect::from_x_y_ranges(column, row.y_range()).shrink2(egui::vec2(4.0, 0.0));
    let card = Rect::from_x_y_ranges(column, row.y_range()).shrink2(egui::vec2(0.0, 1.0));
    ui.painter()
        .rect_filled(card, 4.0, ui.visuals().faint_bg_color);
    let editing = renaming
        .as_ref()
        .is_some_and(|edit| edit.track == row.track);
    let wants_line = editing || track.name.is_some();
    // The name takes a line of its own below the switches, when the row has room.
    let (switch_rect, name_rect) = if wants_line && row.height >= TWO_LINE_HEIGHT {
        let middle = rect.center().y;
        let (top, bottom) = (rect.with_max_y(middle), rect.with_min_y(middle));
        (top, Some(bottom))
    } else {
        (rect, None)
    };
    let mut action = None;
    let mut started = None;
    let mut finished = None;
    ui.scope_builder(UiBuilder::new().max_rect(switch_rect), |ui| {
        ui.set_clip_rect(rect.intersect(ui.clip_rect()));
        ui.horizontal_centered(|ui| {
            ui.set_min_height(switch_rect.height());
            let targeted = targets.contains(row.track);
            let hover = match &track.name {
                Some(name) => format!("{name}\nTarget this track for Insert and Overwrite"),
                None => "Target this track for Insert and Overwrite".to_owned(),
            };
            let label = ui
                .selectable_label(targeted, RichText::new(row.track.to_string()).strong())
                .on_hover_text(hover);
            if label.clicked() {
                action = Some(HeaderAction::ToggleTarget(row.track));
            }
            label.context_menu(|ui| {
                if ui.button("Rename…").clicked() {
                    started = Some(track.name.clone().unwrap_or_default());
                }
                ui.separator();
                if ui.button("Add Track").clicked() {
                    action = Some(HeaderAction::Add(row.track.kind));
                }
                if ui.button("Delete Track").clicked() {
                    action = Some(HeaderAction::Delete(row.track));
                }
            });
            if let (Some(edit), None) = (renaming.as_mut(), name_rect) {
                if edit.track == row.track {
                    finished = name_field(ui, edit);
                    return;
                }
            }
            if let Some(switched) = switches(ui, row, track, automation) {
                action = Some(switched);
            }
        });
    });
    if let Some(name_rect) = name_rect {
        ui.scope_builder(UiBuilder::new().max_rect(name_rect), |ui| {
            ui.set_clip_rect(rect.intersect(ui.clip_rect()));
            ui.horizontal_centered(|ui| {
                ui.set_min_height(name_rect.height());
                match renaming.as_mut().filter(|edit| edit.track == row.track) {
                    Some(edit) => finished = name_field(ui, edit),
                    None => {
                        let name = track.name.as_deref().unwrap_or_default();
                        let text = RichText::new(name).weak();
                        let label = ui.add(Label::new(text).truncate().sense(Sense::click()));
                        if label.double_clicked() {
                            started = Some(name.to_owned());
                        }
                    }
                }
            });
        });
    }
    if let Some(text) = started {
        *renaming = Some(Renaming {
            track: row.track,
            text,
            focused: false,
        });
    }
    if let Some(name) = finished {
        *renaming = None;
        action = name.map(|name| HeaderAction::Rename(row.track, name));
    }
    action
}

/// Mute or hide, lock, sync lock and, for audio, solo and the automation toggle.
fn switches(ui: &mut egui::Ui, row: &Row, track: &Track, automation: bool) -> Option<HeaderAction> {
    let mut action = None;
    ui.spacing_mut().item_spacing.x = 2.0;
    let output = match row.track.kind {
        TrackKind::Video => (Icon::Eye, "Toggle track output"),
        TrackKind::Audio => (Icon::Speaker, "Mute track"),
    };
    let solo = (row.track.kind == TrackKind::Audio).then_some((
        Icon::Solo,
        "Solo track",
        TrackSwitch::Solo,
        track.solo,
    ));
    let switches = [
        (output.0, output.1, TrackSwitch::Mute, track.muted),
        (Icon::Lock, "Lock track", TrackSwitch::Lock, track.locked),
        (
            Icon::Link,
            "Sync lock: inserts and ripples on other tracks shift this one too",
            TrackSwitch::SyncLock,
            !track.sync_unlocked,
        ),
    ];
    for (icon, hover, switch, on) in switches.into_iter().chain(solo) {
        let look = match switch {
            TrackSwitch::Mute => Look::Crossed,
            TrackSwitch::SyncLock => Look::Quiet,
            _ => Look::Filled,
        };
        if icons::toggle(ui, on, icon, look)
            .on_hover_text(hover)
            .clicked()
        {
            action = Some(HeaderAction::Switch(row.track, switch, !on));
        }
    }
    if row.track.kind == TrackKind::Audio {
        let toggle = icons::toggle(ui, automation, Icon::Keyframes, Look::Filled)
            .on_hover_text("Show and edit the track's volume keyframes");
        if toggle.clicked() {
            action = Some(HeaderAction::ToggleAutomation(row.track));
        }
    }
    action
}

/// The field a name is typed into, filling the rest of the row. Returns once typing
/// ends: the name on Enter or clicking away, none on Escape.
fn name_field(ui: &mut egui::Ui, renaming: &mut Renaming) -> Option<Option<String>> {
    let field = ui.add(
        TextEdit::singleline(&mut renaming.text)
            .hint_text("Track name")
            .desired_width(ui.available_width()),
    );
    if !renaming.focused {
        field.request_focus();
        renaming.focused = true;
        return None;
    }
    if !field.lost_focus() {
        return None;
    }
    let cancelled = ui.input(|input| input.key_pressed(egui::Key::Escape));
    Some((!cancelled).then(|| renaming.text.clone()))
}
