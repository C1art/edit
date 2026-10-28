use egui::RichText;
use timeline::{AudioEffect, Edit, SetTrackEffects};

use super::{live, submixes, Change, Channel};
use crate::editor::Editor;

/// The strip whose inserts are open below the strips, and their settings while a
/// control is being dragged.
#[derive(Debug, Default)]
pub(super) struct Inserts {
    open: Option<Channel>,
    adjusting: Option<(Channel, Vec<AudioEffect>)>,
}

/// A strip's button for its inserts, naming how many there are.
pub(super) fn toggle(editor: &mut Editor, ui: &mut egui::Ui, channel: Channel, count: usize) {
    let inserts = &mut editor.mixer.inserts;
    let open = inserts.open == Some(channel);
    let label = match count {
        0 => "fx".to_owned(),
        count => format!("fx {count}"),
    };
    let tip = "Effects on everything this strip mixes, before its fader";
    if ui
        .selectable_label(open, label)
        .on_hover_text(tip)
        .clicked()
    {
        inserts.open = (!open).then_some(channel);
        inserts.adjusting = None;
    }
}

/// The open strip's inserts, as Premiere's mixer shows a selected effect's settings.
pub(super) fn panel(editor: &mut Editor, ui: &mut egui::Ui) {
    let Some(channel) = editor.mixer.inserts.open else {
        return;
    };
    let Some((name, current)) = owned(editor, channel) else {
        editor.mixer.inserts.open = None;
        return;
    };
    let mut effects = match &editor.mixer.inserts.adjusting {
        Some((adjusting, effects)) if *adjusting == channel => effects.clone(),
        _ => current,
    };
    ui.separator();
    ui.label(RichText::new(format!("{name} Effects")).strong());
    let (change, seek) = crate::effects::track_effect_controls(ui, &mut effects, editor.playhead());
    if let Some(key) = seek {
        editor.seek_program(key);
    }
    let Some(change) = change else {
        return;
    };
    editor.mixer.inserts.adjusting = (change == Change::Live).then(|| (channel, effects.clone()));
    match channel {
        Channel::Track(track) => settle(editor, change, SetTrackEffects { track, effects }),
        Channel::Submix(id) => {
            if let Some(edit) = submixes::changed(editor, id, |submix| submix.effects = effects) {
                settle(editor, change, edit);
            }
        }
        Channel::Master => {}
    }
}

fn settle(editor: &mut Editor, change: Change, edit: impl Edit + 'static) {
    match change {
        Change::Live => live(editor, &edit),
        Change::Commit => editor.apply_levels(edit),
    }
}

/// The strip's name and inserts; none for the master, which has none.
fn owned(editor: &Editor, channel: Channel) -> Option<(String, Vec<AudioEffect>)> {
    match channel {
        Channel::Track(track) => {
            let effects = editor.timeline.sequence().track(track)?.effects.clone();
            Some((track.to_string(), effects))
        }
        Channel::Submix(id) => {
            let submix = submixes::find(editor, id)?;
            Some((submix.name.clone(), submix.effects.clone()))
        }
        Channel::Master => None,
    }
}
