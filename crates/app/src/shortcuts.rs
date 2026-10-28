use egui::{Key, KeyboardShortcut, Modifiers};
use timeline::{PlaceMode, TrackKind};

use crate::editor::Editor;
use crate::monitors::Focus;
use crate::placing;
use crate::project_files::{AfterDiscard, PathPurpose};
use crate::timeline_view::Tool;
use crate::transport;

pub const NEW: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::ALT), Key::N);
pub const OPEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
pub const SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
pub const SAVE_AS: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::S);
pub const IMPORT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::I);
pub const EXPORT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::M);
pub const UNDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
pub const REDO: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::Z);
const REDO_ALT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);
pub const ADD_EDIT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::K);
pub const TRANSITION: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::D);
pub const AUDIO_TRANSITION: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::D);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    NewProject,
    OpenProject,
    Save,
    SaveAs,
    Import,
    Export,
    Undo,
    Redo,
    AddEdit,
    Transition(TrackKind),
    RippleDelete,
    Lift,
    MarkIn,
    MarkOut,
    Place(PlaceMode),
    Tool(Tool),
    ToggleSnapping,
    ZoomIn,
    ZoomOut,
    ZoomToFit,
    CancelDrag,
    ClipGain,
}

/// Editor-wide keys. Skipped while a text field has focus so typing isn't hijacked.
pub fn handle(editor: &mut Editor, ctx: &egui::Context) {
    if ctx.egui_wants_keyboard_input() {
        return;
    }
    if let Some(action) = pressed(ctx) {
        run(editor, action);
    }
    if let Some(key) = transport::pressed_key(ctx) {
        let player = match editor.focus {
            Focus::Source => editor
                .source
                .as_mut()
                .map(|source| &mut source.viewer.player),
            Focus::Program => editor.program.as_mut().map(|program| &mut program.player),
        };
        if let Some(player) = player {
            transport::handle_key(player, key);
        }
    }
}

fn pressed(ctx: &egui::Context) -> Option<Action> {
    ctx.input_mut(|input| {
        // Shortcuts with more modifiers go first so a plainer one can't claim their key.
        let chords = [
            (NEW, Action::NewProject),
            (SAVE_AS, Action::SaveAs),
            (REDO, Action::Redo),
            (REDO_ALT, Action::Redo),
            (AUDIO_TRANSITION, Action::Transition(TrackKind::Audio)),
            (UNDO, Action::Undo),
            (OPEN, Action::OpenProject),
            (SAVE, Action::Save),
            (IMPORT, Action::Import),
            (EXPORT, Action::Export),
            (ADD_EDIT, Action::AddEdit),
            (TRANSITION, Action::Transition(TrackKind::Video)),
            (
                KeyboardShortcut::new(Modifiers::SHIFT, Key::Delete),
                Action::RippleDelete,
            ),
        ];
        if let Some(action) = chords
            .into_iter()
            .find_map(|(chord, action)| input.consume_shortcut(&chord).then_some(action))
        {
            return Some(action);
        }
        let tools = Tool::ALL.map(|tool| (tool.key(), Action::Tool(tool)));
        let keys = [
            (Key::Delete, Action::Lift),
            (Key::Backspace, Action::Lift),
            (Key::I, Action::MarkIn),
            (Key::O, Action::MarkOut),
            (Key::Comma, Action::Place(PlaceMode::Insert)),
            (Key::Period, Action::Place(PlaceMode::Overwrite)),
            (Key::S, Action::ToggleSnapping),
            (Key::Equals, Action::ZoomIn),
            (Key::Plus, Action::ZoomIn),
            (Key::Minus, Action::ZoomOut),
            (Key::Backslash, Action::ZoomToFit),
            (Key::Escape, Action::CancelDrag),
            (Key::G, Action::ClipGain),
        ];
        keys.into_iter()
            .chain(tools)
            .find_map(|(key, action)| input.consume_key(Modifiers::NONE, key).then_some(action))
    })
}

fn run(editor: &mut Editor, action: Action) {
    match action {
        Action::NewProject => editor.discard_then(AfterDiscard::NewProject),
        Action::OpenProject => editor.prompt_path(PathPurpose::OpenProject),
        Action::Save => editor.save_project(),
        Action::SaveAs => editor.prompt_path(PathPurpose::SaveProject),
        Action::Import => editor.prompt_path(PathPurpose::Import),
        Action::Export => crate::export::open(editor),
        Action::Undo => editor.undo(),
        Action::Redo => editor.redo(),
        Action::AddEdit => editor.add_edit(),
        Action::Transition(kind) => crate::effects::apply_default_transition(editor, kind),
        Action::RippleDelete => editor.ripple_delete(),
        Action::Lift => editor.lift(),
        Action::MarkIn | Action::MarkOut => {
            if let Some(source) = &mut editor.source {
                if action == Action::MarkIn {
                    source.mark_in();
                } else {
                    source.mark_out();
                }
            }
        }
        Action::Place(mode) => placing::edit_from_source(editor, mode),
        Action::Tool(tool) => editor.view.tool = tool,
        Action::ToggleSnapping => editor.view.snapping = !editor.view.snapping,
        Action::ZoomIn => editor.view.zoom(2.0, None),
        Action::ZoomOut => editor.view.zoom(0.5, None),
        Action::ZoomToFit => editor.view.zoom_to_fit(editor.timeline.sequence()),
        Action::CancelDrag => editor.view.cancel_drag(),
        Action::ClipGain => editor.prompt_clip_gain(),
    }
}
