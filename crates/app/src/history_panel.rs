use egui::RichText;

use crate::editor::Editor;

/// The state before any edit, at the top of the list.
const START: &str = "Start";

/// Every undoable step, oldest first, the current one highlighted and undone ones
/// dimmed; clicking a step undoes or redoes back to it.
pub fn ui(editor: &mut Editor, ui: &mut egui::Ui) {
    let timeline = &editor.timeline;
    let current = timeline.done().count();
    let steps: Vec<&str> = std::iter::once(START)
        .chain(timeline.done())
        .chain(timeline.undone())
        .collect();
    let mut chosen = None;
    egui::ScrollArea::vertical()
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for (step, name) in steps.into_iter().enumerate() {
                let text = RichText::new(name);
                let text = if step > current { text.weak() } else { text };
                let row = ui.selectable_label(step == current, text);
                if step == current && editor.history_scrolled != Some(current) {
                    row.scroll_to_me(None);
                    editor.history_scrolled = Some(current);
                }
                if row.clicked() && step != current {
                    chosen = Some(step);
                }
            }
        });
    if let Some(step) = chosen {
        editor.go_to_history(step);
    }
}
