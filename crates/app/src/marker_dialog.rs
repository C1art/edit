use egui::Key;
use timeline::{
    AddMarker, Marker, MarkerColor, MarkerId, MarkerOwner, RemoveMarker, Ticks, Timecode,
    UpdateMarker,
};

use crate::editor::Editor;
use crate::timeline_view::marker_color;

impl Editor {
    /// Where M puts a marker: on the first selected clip under the playhead, in its
    /// source time, or else on the sequence.
    fn marker_target(&self) -> (MarkerOwner, Ticks) {
        let playhead = self.playhead();
        let sequence = self.timeline.sequence();
        self.view
            .selection
            .iter()
            .filter_map(|&id| sequence.clip(id))
            .find(|(_, clip)| clip.covers(playhead))
            .map_or((MarkerOwner::Sequence, playhead), |(_, clip)| {
                (MarkerOwner::Clip(clip.id), clip.source_time(playhead))
            })
    }

    fn markers_of(&self, owner: MarkerOwner) -> &[Marker] {
        let sequence = self.timeline.sequence();
        match owner {
            MarkerOwner::Sequence => &sequence.markers,
            MarkerOwner::Clip(id) => sequence.clip(id).map_or(&[], |(_, clip)| &clip.markers),
        }
    }

    /// Adds a marker at the playhead, or opens the one already there.
    pub(crate) fn add_marker(&mut self) {
        let (owner, at) = self.marker_target();
        match self.marker_at_playhead() {
            Some(found) => self.edit_marker(found),
            None => {
                self.apply(AddMarker { owner, at });
            }
        }
    }

    pub(crate) fn edit_marker(&mut self, (owner, id): (MarkerOwner, MarkerId)) {
        self.marker_dialog = self
            .markers_of(owner)
            .iter()
            .find(|marker| marker.id == id)
            .map(|marker| (owner, marker.clone()));
    }

    pub(crate) fn clear_marker(&mut self, (owner, id): (MarkerOwner, MarkerId)) {
        self.apply(RemoveMarker { owner, id });
    }

    /// Moves the program playhead to the next sequence marker, or the previous one.
    pub(crate) fn jump_to_marker(&mut self, forward: bool) {
        let sequence = self.timeline.sequence();
        let playhead = self.playhead();
        let marker = if forward {
            sequence.marker_after(playhead)
        } else {
            sequence.marker_before(playhead)
        };
        if let Some(at) = marker.map(|marker| marker.at) {
            self.seek_program(at);
        }
    }

    /// The marker at the playhead on whatever M would mark, if any.
    pub(crate) fn marker_at_playhead(&self) -> Option<(MarkerOwner, MarkerId)> {
        let (owner, at) = self.marker_target();
        self.markers_of(owner)
            .iter()
            .find(|marker| marker.at == at)
            .map(|marker| (owner, marker.id))
    }
}

pub fn dialog(editor: &mut Editor, ctx: &egui::Context) {
    let sequence = editor.timeline.sequence();
    let (timebase, rate) = (sequence.timebase, sequence.format.frame_rate);
    let Some((owner, marker)) = &mut editor.marker_dialog else {
        return;
    };
    let mut choice = None;
    egui::Window::new("Marker")
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            egui::Grid::new("marker_fields")
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("Name");
                    let name = ui.text_edit_singleline(&mut marker.name);
                    if ui.memory(|memory| memory.focused().is_none()) {
                        name.request_focus();
                    }
                    ui.end_row();
                    ui.label(match owner {
                        MarkerOwner::Sequence => "Time",
                        MarkerOwner::Clip(_) => "Source Time",
                    });
                    ui.monospace(Timecode::new(marker.at, timebase, rate).to_string());
                    ui.end_row();
                    ui.label("Color");
                    color_picker(ui, &mut marker.color);
                    ui.end_row();
                    ui.label("Comments");
                    ui.add(egui::TextEdit::multiline(&mut marker.comment).desired_rows(3));
                    ui.end_row();
                });
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() {
                    choice = Some(Choice::Apply);
                }
                if ui.button("Delete").clicked() {
                    choice = Some(Choice::Delete);
                }
                if ui.button("Cancel").clicked() || ui.input(|input| input.key_pressed(Key::Escape))
                {
                    choice = Some(Choice::Cancel);
                }
            });
        });
    let Some(choice) = choice else {
        return;
    };
    let Some((owner, marker)) = editor.marker_dialog.take() else {
        return;
    };
    match choice {
        Choice::Apply => {
            editor.apply(UpdateMarker { owner, marker });
        }
        Choice::Delete => editor.clear_marker((owner, marker.id)),
        Choice::Cancel => {}
    }
}

fn color_picker(ui: &mut egui::Ui, selected: &mut MarkerColor) {
    ui.horizontal(|ui| {
        for color in MarkerColor::ALL {
            let size = egui::vec2(16.0, 16.0);
            let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
            let fill = marker_color(color);
            ui.painter().rect_filled(rect.shrink(2.0), 2.0, fill);
            if *selected == color {
                ui.painter().rect_stroke(
                    rect,
                    2.0,
                    egui::Stroke::new(1.5, ui.visuals().strong_text_color()),
                    egui::StrokeKind::Inside,
                );
            }
            if response.on_hover_text(color.name()).clicked() {
                *selected = color;
            }
        }
    });
}

enum Choice {
    Apply,
    Delete,
    Cancel,
}

pub fn menu(editor: &mut Editor, ui: &mut egui::Ui) {
    use crate::shortcuts;
    let ctx = ui.ctx().clone();
    let shortcut = |chord| ctx.format_shortcut(&chord);
    let add = egui::Button::new("Add Marker").shortcut_text(shortcut(shortcuts::ADD_MARKER));
    if ui.add(add).clicked() {
        editor.add_marker();
    }
    let next =
        egui::Button::new("Go to Next Marker").shortcut_text(shortcut(shortcuts::NEXT_MARKER));
    if ui.add(next).clicked() {
        editor.jump_to_marker(true);
    }
    let previous = egui::Button::new("Go to Previous Marker")
        .shortcut_text(shortcut(shortcuts::PREVIOUS_MARKER));
    if ui.add(previous).clicked() {
        editor.jump_to_marker(false);
    }
    let here = editor.marker_at_playhead();
    if ui
        .add_enabled(here.is_some(), egui::Button::new("Edit Marker…"))
        .clicked()
    {
        if let Some(id) = here {
            editor.edit_marker(id);
        }
    }
    let clear = egui::Button::new("Clear Marker").shortcut_text(shortcut(shortcuts::CLEAR_MARKER));
    if ui.add_enabled(here.is_some(), clear).clicked() {
        if let Some(id) = here {
            editor.clear_marker(id);
        }
    }
}
