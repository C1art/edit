use egui::{ComboBox, RichText, TextEdit};
use timeline::{AddSubmix, RemoveSubmix, SetSubmix, SetTrackOutput, Submix, SubmixId, TrackRef};

use crate::editor::Editor;

/// `id` with `change` made to it, as an edit; none once it's gone.
pub(super) fn changed(
    editor: &Editor,
    id: SubmixId,
    change: impl FnOnce(&mut Submix),
) -> Option<SetSubmix> {
    let mut submix = find(editor, id)?.clone();
    change(&mut submix);
    Some(SetSubmix { submix })
}

pub(super) fn find(editor: &Editor, id: SubmixId) -> Option<&Submix> {
    let submixes = &editor.timeline.sequence().submixes;
    submixes.iter().find(|submix| submix.id == id)
}

/// The submix's name, renamed in place after a double-click; its menu deletes it.
pub(super) fn title(editor: &mut Editor, ui: &mut egui::Ui, id: SubmixId, name: &str) {
    if let Some((renaming, text, focused)) = &mut editor.mixer.renaming {
        if *renaming == id {
            let field = ui.add(TextEdit::singleline(text).desired_width(60.0));
            if !*focused {
                field.request_focus();
                *focused = true;
            }
            if field.lost_focus() {
                let name = text.trim().to_owned();
                editor.mixer.renaming = None;
                let entered =
                    !name.is_empty() && !ui.input(|input| input.key_pressed(egui::Key::Escape));
                if let Some(edit) =
                    changed(editor, id, |submix| submix.name = name).filter(|_| entered)
                {
                    editor.apply(edit);
                }
            }
            return;
        }
    }
    let label = ui
        .label(RichText::new(name).strong())
        .on_hover_text("Double-click to rename; right-click for more");
    let rename = || Some((id, name.to_owned(), false));
    if label.double_clicked() {
        editor.mixer.renaming = rename();
    }
    label.context_menu(|ui| {
        if ui.button("Rename").clicked() {
            editor.mixer.renaming = rename();
        }
        if ui.button("Delete Submix").clicked() {
            editor.apply(RemoveSubmix { submix: id });
        }
    });
}

/// Where the track's sound goes: the master or one of the submixes.
pub(super) fn output_picker(editor: &mut Editor, ui: &mut egui::Ui, track: TrackRef) {
    let sequence = editor.timeline.sequence();
    let Some(current) = sequence.track(track) else {
        return;
    };
    let mut output = current
        .output
        .filter(|&id| sequence.submixes.iter().any(|submix| submix.id == id));
    let shown = output
        .and_then(|id| find(editor, id))
        .map_or("Master", |submix| submix.name.as_str());
    let before = output;
    ComboBox::from_id_salt(("track output", track))
        .width(60.0)
        .wrap_mode(egui::TextWrapMode::Extend)
        .selected_text(shown)
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut output, None, "Master");
            for submix in &sequence.submixes {
                ui.selectable_value(&mut output, Some(submix.id), &submix.name);
            }
        })
        .response
        .on_hover_text("Track output");
    if output != before {
        editor.apply(SetTrackOutput { track, output });
    }
}

pub(super) fn add_button(editor: &mut Editor, ui: &mut egui::Ui) {
    let tip = "A bus to send tracks to, with its own fader and effects";
    if ui.button("+ Submix").on_hover_text(tip).clicked() {
        editor.apply(AddSubmix);
    }
}
