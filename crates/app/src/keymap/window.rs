use egui::{Button, Color32, Key, RichText, ScrollArea, TextEdit};

use super::{take_press, Chord, Command, Keymap};
use crate::editor::Editor;

const CLASH: Color32 = Color32::from_rgb(230, 150, 60);

#[derive(Debug, Default)]
pub struct KeymapWindow {
    search: String,
    /// The command the next key press is bound to.
    pub(crate) recording: Option<Command>,
    message: Option<String>,
}

/// Lists every command with its keys, as Premiere's Keyboard Shortcuts dialog does:
/// a key is added by pressing it, taken from whatever had it, and removed by clicking.
pub fn window(editor: &mut Editor, ctx: &egui::Context) {
    let Some(state) = &mut editor.keymap_window else {
        return;
    };
    let mut changed = false;
    if let Some(command) = state.recording {
        if let Some(chord) = take_press(ctx, Some) {
            state.recording = None;
            if chord != Chord::new(false, false, false, Key::Escape) {
                state.message = editor.keymap.bind(command, chord).map(|previous| {
                    format!("{} was taken from {}.", chord.label(), previous.name())
                });
                changed = true;
            }
        }
    }
    let mut open = true;
    egui::Window::new("Keyboard Shortcuts")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size([560.0, 520.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let search = TextEdit::singleline(&mut state.search)
                    .hint_text("Search commands or keys")
                    .desired_width(260.0);
                ui.add(search);
                if ui.button("Reset to Defaults").clicked() {
                    editor.keymap = Keymap::default();
                    state.message = Some("Every command has its default keys again.".into());
                    changed = true;
                }
            });
            let hint = "Click + and press a key to add it; click a key to remove it.";
            ui.label(RichText::new(state.message.as_deref().unwrap_or(hint)).weak());
            ui.separator();
            let needle = state.search.to_lowercase();
            ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                egui::Grid::new("keymap")
                    .num_columns(2)
                    .spacing([24.0, 4.0])
                    .show(ui, |ui| {
                        let mut category = "";
                        for command in Command::ALL {
                            let chords = editor.keymap.chords(command).to_vec();
                            if !matches(command, &chords, &needle) {
                                continue;
                            }
                            if command.category() != category {
                                category = command.category();
                                ui.label(RichText::new(category).strong());
                                ui.end_row();
                            }
                            ui.label(command.name());
                            ui.horizontal(|ui| {
                                for chord in chords {
                                    let clash = editor.keymap.clashes(command, chord);
                                    let mut text = RichText::new(format!("{}  ×", chord.label()));
                                    if clash {
                                        text = text.color(CLASH);
                                    }
                                    let tip = if clash {
                                        "Another command has this key too. Click to remove it."
                                    } else {
                                        "Click to remove this key."
                                    };
                                    if ui.small_button(text).on_hover_text(tip).clicked() {
                                        editor.keymap.unbind(command, chord);
                                        changed = true;
                                    }
                                }
                                let recording = state.recording == Some(command);
                                let label = if recording { "Press a key…" } else { "+" };
                                let add = Button::new(label).small().selected(recording);
                                let tip = "Add a key. Escape cancels.";
                                if ui.add(add).on_hover_text(tip).clicked() {
                                    state.recording = (!recording).then_some(command);
                                }
                            });
                            ui.end_row();
                        }
                    });
            });
        });
    if !open {
        editor.keymap_window = None;
    }
    if changed {
        editor.save_keymap();
    }
}

fn matches(command: Command, chords: &[Chord], needle: &str) -> bool {
    let found = |text: &str| text.to_lowercase().contains(needle);
    needle.is_empty()
        || found(command.name())
        || chords
            .iter()
            .any(|chord| found(&chord.label()) || found(&chord.to_string()))
}
