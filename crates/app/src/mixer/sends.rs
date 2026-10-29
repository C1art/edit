use egui::{ComboBox, DragValue, RichText};
use timeline::{Gain, Pan, SendTap, SetTrackSends, TrackRef, TrackSend};

use super::{inserts, response_state, Change, FADER_RANGE};
use crate::editor::Editor;
use crate::icons;

/// The track whose sends are open below the strips, and their settings while a
/// control is being dragged.
#[derive(Debug, Default)]
pub(super) struct Sends {
    open: Option<TrackRef>,
    adjusting: Option<(TrackRef, Vec<TrackSend>)>,
}

/// A track strip's button for its sends, naming how many there are.
pub(super) fn toggle(editor: &mut Editor, ui: &mut egui::Ui, track: TrackRef) {
    let sequence = editor.timeline.sequence();
    let count = sequence.track(track).map_or(0, |track| track.sends.len());
    let sends = &mut editor.mixer.sends;
    let open = sends.open == Some(track);
    let label = match count {
        0 => "sends".to_owned(),
        count => format!("sends {count}"),
    };
    let tip = "Copies of this track's sound for submixes, besides its output";
    if ui
        .selectable_label(open, label)
        .on_hover_text(tip)
        .clicked()
    {
        sends.open = (!open).then_some(track);
        sends.adjusting = None;
    }
}

/// The open track's sends, one row each, as Premiere's mixer shows them.
pub(super) fn panel(editor: &mut Editor, ui: &mut egui::Ui) {
    let Some(track) = editor.mixer.sends.open else {
        return;
    };
    let sequence = editor.timeline.sequence();
    let Some(owner) = sequence.track(track) else {
        editor.mixer.sends.open = None;
        return;
    };
    let name = owner
        .name
        .as_ref()
        .map_or_else(|| track.to_string(), |name| format!("{track} {name}"));
    let submixes: Vec<_> = sequence
        .submixes
        .iter()
        .map(|submix| (submix.id, submix.name.clone()))
        .collect();
    let mut sends = match &editor.mixer.sends.adjusting {
        Some((adjusting, sends)) if *adjusting == track => sends.clone(),
        _ => owner.sends.clone(),
    };
    ui.separator();
    ui.label(RichText::new(format!("{name} Sends")).strong());
    if submixes.is_empty() {
        ui.weak("Add a submix to send this track to.");
        return;
    }
    let mut change = None;
    let mut removed = None;
    egui::Grid::new(("track sends", track))
        .num_columns(6)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            for (index, send) in sends.iter_mut().enumerate() {
                let before = send.to;
                let shown = submixes
                    .iter()
                    .find(|(id, _)| *id == send.to)
                    .map_or("", |(_, name)| name.as_str());
                ComboBox::from_id_salt(("send to", track, index))
                    .width(90.0)
                    .selected_text(shown)
                    .show_ui(ui, |ui| {
                        for (id, name) in &submixes {
                            ui.selectable_value(&mut send.to, *id, name);
                        }
                    });
                if send.to != before {
                    change = Some(Change::Commit);
                }
                let mut db = send.level.db().max(*FADER_RANGE.start());
                let level = ui
                    .add(
                        DragValue::new(&mut db)
                            .range(FADER_RANGE)
                            .speed(0.1)
                            .max_decimals(1)
                            .suffix(" dB"),
                    )
                    .on_hover_text("Send level; double-click for 0 dB");
                if level.double_clicked() {
                    db = 0.0;
                }
                send.level = if db <= *FADER_RANGE.start() {
                    Gain::SILENT
                } else {
                    Gain::from_db(db)
                };
                change = response_state(&level, change);
                let mut balance = send.pan.value();
                let pan = ui
                    .add(
                        DragValue::new(&mut balance)
                            .range(-100..=100)
                            .prefix("Pan "),
                    )
                    .on_hover_text("Send balance; double-click to centre");
                if pan.double_clicked() {
                    balance = 0;
                }
                send.pan = Pan::new(balance);
                change = response_state(&pan, change);
                let tap = send.tap;
                ComboBox::from_id_salt(("send tap", track, index))
                    .width(80.0)
                    .selected_text(tap.name())
                    .show_ui(ui, |ui| {
                        for option in SendTap::ALL {
                            ui.selectable_value(&mut send.tap, option, option.name());
                        }
                    });
                if send.tap != tap {
                    change = Some(Change::Commit);
                }
                if ui
                    .selectable_label(send.muted, "M")
                    .on_hover_text("Mute this send")
                    .clicked()
                {
                    send.muted = !send.muted;
                    change = Some(Change::Commit);
                }
                let remove = icons::button(ui, icons::Icon::Close, egui::vec2(18.0, 18.0));
                if remove.on_hover_text("Remove send").clicked() {
                    removed = Some(index);
                }
                ui.end_row();
            }
        });
    if let Some(index) = removed {
        sends.remove(index);
        change = Some(Change::Commit);
    }
    let room = sends.len() < TrackSend::MAX;
    if ui
        .add_enabled(room, egui::Button::new("+ Send"))
        .on_hover_text("Send this track to a submix too")
        .clicked()
    {
        let unused = submixes
            .iter()
            .map(|&(id, _)| id)
            .find(|&id| sends.iter().all(|send| send.to != id));
        if let Some(to) = unused.or(submixes.first().map(|&(id, _)| id)) {
            sends.push(TrackSend::new(to));
            change = Some(Change::Commit);
        }
    }
    let Some(change) = change else {
        return;
    };
    editor.mixer.sends.adjusting = (change == Change::Live).then(|| (track, sends.clone()));
    inserts::settle(editor, change, SetTrackSends { track, sends });
}
