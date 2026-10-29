use egui::Key;
use timeline::FourPoint;

use crate::editor::Editor;
use crate::placing;

const CHOICES: [(FourPoint, &str, &str); 3] = [
    (
        FourPoint::FitToFill,
        "Change Clip Speed (Fit to Fill)",
        "Plays the whole source range over the sequence range",
    ),
    (
        FourPoint::IgnoreSourceOut,
        "Ignore Source Out Point",
        "Takes as much of the source as the sequence range lasts",
    ),
    (
        FourPoint::IgnoreSequenceOut,
        "Ignore Sequence Out Point",
        "Places the whole source range from the sequence in point",
    ),
];

/// Premiere's Fit Clip dialog, for a source range and sequence range of different
/// lengths.
pub(crate) fn dialog(editor: &mut Editor, ctx: &egui::Context) {
    let Some(mode) = editor.fit_clip else {
        return;
    };
    let mut chosen = None;
    let mut cancelled = ctx.input(|input| input.key_pressed(Key::Escape));
    crate::dialog::window(ctx, "Fit Clip").show(ctx, |ui| {
        ui.label("The source and sequence ranges are different lengths.");
        ui.add_space(4.0);
        for (choice, label, hint) in CHOICES {
            if ui.button(label).on_hover_text(hint).clicked() {
                chosen = Some(choice);
            }
        }
        crate::dialog::footer(ui, |ui| cancelled |= ui.button("Cancel").clicked());
    });
    if chosen.is_some() || cancelled {
        editor.fit_clip = None;
    }
    if let Some(choice) = chosen {
        placing::place_from_source(editor, mode, Some(choice));
    }
}
