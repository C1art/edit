use egui::Key;
use timeline::{next_found, ClipId, FindOperator, FindQuery, Found, Ticks};

use crate::editor::Editor;

/// Edit > Find, which keeps its query between uses as Premiere's does.
#[derive(Default)]
pub(crate) struct FindDialog {
    pub(crate) open: bool,
    query: FindQuery,
    focus_text: bool,
}

enum Action {
    Next,
    All,
}

impl Editor {
    pub(crate) fn open_find(&mut self) {
        self.find.open = true;
        self.find.focus_text = true;
    }

    fn found(&self) -> Vec<(Ticks, Found)> {
        let sequence = self.timeline.sequence();
        sequence.find(self.timeline.project(), &self.find.query)
    }

    /// Moves the playhead to the next match after it, selecting it when it is a clip.
    fn find_next(&mut self) {
        let Some((at, found)) = next_found(&self.found(), self.playhead()) else {
            self.notice("Nothing in the sequence matches");
            return;
        };
        if let Found::Clip(clip) = found {
            self.view.selection = vec![clip];
        }
        self.seek_program(at);
    }

    fn find_all(&mut self) {
        let clips: Vec<ClipId> = self
            .found()
            .into_iter()
            .filter_map(|(_, found)| match found {
                Found::Clip(clip) => Some(clip),
                Found::Marker(_) => None,
            })
            .collect();
        self.notice(format!("Selected {} matching clip(s)", clips.len()));
        self.view.selection = clips;
    }
}

pub(crate) fn dialog(editor: &mut Editor, ctx: &egui::Context) {
    if !editor.find.open {
        return;
    }
    let mut open = true;
    let mut done = false;
    let mut action = None;
    let find = &mut editor.find;
    crate::dialog::window(ctx, "Find")
        .open(&mut open)
        .show(ctx, |ui| {
            egui::Grid::new("find").num_columns(2).show(ui, |ui| {
                ui.label("Find");
                let field = ui
                    .text_edit_singleline(&mut find.query.text)
                    .on_hover_text("Matches clip names and sequence marker names and comments");
                if crate::dialog::opening(ui, &mut find.focus_text) {
                    crate::dialog::focus_text(ui, &field, &find.query.text);
                }
                if field.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter)) {
                    action = Some(Action::Next);
                }
                ui.end_row();
                ui.label("Operator");
                let operator = &mut find.query.operator;
                egui::ComboBox::from_id_salt("find_operator")
                    .selected_text(operator.name())
                    .show_ui(ui, |ui| {
                        for option in FindOperator::ALL {
                            ui.selectable_value(operator, option, option.name());
                        }
                    });
                ui.end_row();
            });
            ui.checkbox(&mut find.query.match_case, "Match Case");
            crate::dialog::footer(ui, |ui| {
                if ui.button("Done").clicked() || ui.input(|input| input.key_pressed(Key::Escape)) {
                    done = true;
                }
                if ui.button("Find All").clicked() {
                    action = Some(Action::All);
                }
                if ui.add(crate::dialog::primary("Find")).clicked() {
                    action = Some(Action::Next);
                }
            });
        });
    editor.find.open = open && !done;
    match action {
        Some(Action::Next) => editor.find_next(),
        Some(Action::All) => editor.find_all(),
        None => {}
    }
}
