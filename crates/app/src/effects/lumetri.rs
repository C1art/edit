use egui::RichText;
use timeline::TrackKind;

use super::{clip_name, picture_controls, Sections};
use crate::editor::Editor;

/// The color controls of the first selected video clip, open and on their own.
pub fn ui(editor: &mut Editor, ui: &mut egui::Ui) {
    let sequence = editor.timeline.sequence();
    let picture = editor
        .view
        .selection
        .iter()
        .filter_map(|&id| sequence.clip(id))
        .find(|(track, _)| track.kind == TrackKind::Video)
        .map(|(_, clip)| (clip.clone(), clip_name(sequence, clip).to_owned()));
    let Some((clip, name)) = picture else {
        ui.label("Select a video clip to grade it.");
        return;
    };
    ui.add(egui::Label::new(RichText::new(name).strong()).truncate());
    egui::ScrollArea::vertical().show(ui, |ui| {
        // Sections nest a level or two deep, and the panel is often narrow.
        ui.spacing_mut().indent = 10.0;
        picture_controls(editor, ui, &clip, Sections::Color);
    });
    editor.effects.keys.end_frame(ui, clip.id);
}
