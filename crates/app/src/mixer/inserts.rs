use egui::RichText;
use timeline::{AudioEffect, SetTrackEffects, TrackRef};

use super::{live, Change};
use crate::editor::Editor;

/// The track whose inserts are open below the strips, and their settings while a
/// control is being dragged.
#[derive(Debug, Default)]
pub(super) struct Inserts {
    open: Option<TrackRef>,
    adjusting: Option<(TrackRef, Vec<AudioEffect>)>,
}

/// A strip's button for its inserts, naming how many there are.
pub(super) fn toggle(editor: &mut Editor, ui: &mut egui::Ui, track: TrackRef, count: usize) {
    let inserts = &mut editor.mixer.inserts;
    let open = inserts.open == Some(track);
    let label = match count {
        0 => "fx".to_owned(),
        count => format!("fx {count}"),
    };
    let tip = "Effects on the whole track, before its fader";
    if ui
        .selectable_label(open, label)
        .on_hover_text(tip)
        .clicked()
    {
        inserts.open = (!open).then_some(track);
        inserts.adjusting = None;
    }
}

/// The open track's inserts, as Premiere's mixer shows a selected effect's settings.
pub(super) fn panel(editor: &mut Editor, ui: &mut egui::Ui) {
    let Some(track) = editor.mixer.inserts.open else {
        return;
    };
    let Some(current) = editor.timeline.sequence().track(track) else {
        editor.mixer.inserts.open = None;
        return;
    };
    let mut effects = match &editor.mixer.inserts.adjusting {
        Some((adjusting, effects)) if *adjusting == track => effects.clone(),
        _ => current.effects.clone(),
    };
    ui.separator();
    ui.label(RichText::new(format!("{track} Track Effects")).strong());
    let (change, seek) = crate::effects::track_effect_controls(ui, &mut effects, editor.playhead());
    if let Some(key) = seek {
        editor.seek_program(key);
    }
    let edit = SetTrackEffects { track, effects };
    match change {
        Some(Change::Live) => {
            live(editor, &edit);
            editor.mixer.inserts.adjusting = Some((track, edit.effects));
        }
        Some(Change::Commit) => {
            editor.mixer.inserts.adjusting = None;
            editor.apply_levels(edit);
        }
        None => {}
    }
}
