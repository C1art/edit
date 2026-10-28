use egui::{Event, Key, KeyboardShortcut, Modifiers};
use timeline::{Edge, Generator, Marks, PlaceMode, Title, TrackKind};

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
pub const COPY: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::C);
pub const CUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::X);
pub const PASTE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::V);
pub const PASTE_INSERT: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::V);
pub const PASTE_ATTRIBUTES: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::ALT), Key::V);
pub const ADD_MARKER: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::M);
pub const NEXT_MARKER: KeyboardShortcut = KeyboardShortcut::new(Modifiers::SHIFT, Key::M);
pub const PREVIOUS_MARKER: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::M);
pub const CLEAR_MARKER: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::ALT), Key::M);
pub const SPEED_DURATION: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::R);
pub const TOGGLE_ENABLED: KeyboardShortcut = KeyboardShortcut::new(Modifiers::SHIFT, Key::E);
pub const TOGGLE_LINK: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::L);
pub const NEW_TITLE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::T);
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
pub const MULTICAM_VIEW: KeyboardShortcut = KeyboardShortcut::new(Modifiers::SHIFT, Key::Num0);
/// Switch to cameras one to nine in the multi-camera view.
const ANGLE_KEYS: [Key; crate::multicam::KEYED_ANGLES] = [
    Key::Num1,
    Key::Num2,
    Key::Num3,
    Key::Num4,
    Key::Num5,
    Key::Num6,
    Key::Num7,
    Key::Num8,
    Key::Num9,
];
pub const GO_TO_IN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::SHIFT, Key::I);
pub const GO_TO_OUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::SHIFT, Key::O);
pub const CLEAR_IN_OUT: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::X);

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
    Copy,
    Cut,
    AddMarker,
    JumpToMarker { forward: bool },
    ClearMarker,
    Paste(PlaceMode),
    AddEdit,
    Transition(TrackKind),
    RippleDelete,
    Lift,
    MarkIn,
    MarkOut,
    ClearInOut,
    GoToMark(Edge),
    RippleTrimToPlayhead(Edge),
    MatchFrame,
    LiftMarked,
    ExtractMarked,
    Place(PlaceMode),
    Tool(Tool),
    ToggleSnapping,
    ZoomIn,
    ZoomOut,
    ZoomToFit,
    CancelDrag,
    ClipGain,
    SpeedDuration,
    ToggleEnabled,
    ToggleLink,
    PasteAttributes,
    NewTitle,
    ToggleMulticamView,
}

/// Editor-wide keys. Skipped while a text field has focus so typing isn't hijacked.
pub fn handle(editor: &mut Editor, ctx: &egui::Context) {
    if ctx.egui_wants_keyboard_input() {
        return;
    }
    if editor.multicam_layout.is_some() {
        let angle = ctx.input_mut(|input| {
            ANGLE_KEYS
                .iter()
                .position(|&key| input.consume_key(Modifiers::NONE, key))
        });
        if let Some(angle) = angle {
            editor.switch_angle(angle);
        }
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
        if let Some(action) = clipboard_action(input) {
            return Some(action);
        }
        // Shortcuts with more modifiers go first so a plainer one can't claim their key.
        let chords = [
            (NEW, Action::NewProject),
            (SAVE_AS, Action::SaveAs),
            (PASTE_ATTRIBUTES, Action::PasteAttributes),
            (PASTE_INSERT, Action::Paste(PlaceMode::Insert)),
            (CLEAR_MARKER, Action::ClearMarker),
            (PREVIOUS_MARKER, Action::JumpToMarker { forward: false }),
            (NEXT_MARKER, Action::JumpToMarker { forward: true }),
            (REDO, Action::Redo),
            (REDO_ALT, Action::Redo),
            (AUDIO_TRANSITION, Action::Transition(TrackKind::Audio)),
            (UNDO, Action::Undo),
            (OPEN, Action::OpenProject),
            (SAVE, Action::Save),
            (IMPORT, Action::Import),
            (EXPORT, Action::Export),
            (ADD_EDIT, Action::AddEdit),
            (SPEED_DURATION, Action::SpeedDuration),
            (TOGGLE_ENABLED, Action::ToggleEnabled),
            (TOGGLE_LINK, Action::ToggleLink),
            (NEW_TITLE, Action::NewTitle),
            (TRANSITION, Action::Transition(TrackKind::Video)),
            (CLEAR_IN_OUT, Action::ClearInOut),
            (MULTICAM_VIEW, Action::ToggleMulticamView),
            (GO_TO_IN, Action::GoToMark(Edge::Head)),
            (GO_TO_OUT, Action::GoToMark(Edge::Tail)),
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
        let keys = [
            (Key::Delete, Action::Lift),
            (Key::Backspace, Action::Lift),
            (Key::I, Action::MarkIn),
            (Key::O, Action::MarkOut),
            (Key::Q, Action::RippleTrimToPlayhead(Edge::Head)),
            (Key::W, Action::RippleTrimToPlayhead(Edge::Tail)),
            (Key::F, Action::MatchFrame),
            (Key::Semicolon, Action::LiftMarked),
            (Key::Quote, Action::ExtractMarked),
            (Key::Comma, Action::Place(PlaceMode::Insert)),
            (Key::Period, Action::Place(PlaceMode::Overwrite)),
            (Key::S, Action::ToggleSnapping),
            (Key::Equals, Action::ZoomIn),
            (Key::Plus, Action::ZoomIn),
            (Key::Minus, Action::ZoomOut),
            (Key::Backslash, Action::ZoomToFit),
            (Key::Escape, Action::CancelDrag),
            (Key::G, Action::ClipGain),
            (Key::M, Action::AddMarker),
        ];
        let mut tools = Tool::ALL;
        // Matching ignores extra Shift, so A alone would take Shift+A if tried first.
        tools.sort_by_key(|tool| tool.shortcut().0.is_none());
        let tool = tools.into_iter().find_map(|tool| {
            let (modifiers, key) = tool.shortcut();
            input
                .consume_key(modifiers, key)
                .then_some(Action::Tool(tool))
        });
        tool.or_else(|| {
            keys.into_iter()
                .find_map(|(key, action)| input.consume_key(Modifiers::NONE, key).then_some(action))
        })
    })
}

/// The windowing layer turns clipboard chords into events instead of key presses,
/// including Shift+Delete on Windows, which is Ripple Delete here, and Ctrl+Shift+X,
/// which clears in and out.
fn clipboard_action(input: &egui::InputState) -> Option<Action> {
    let shift_only = input.modifiers.shift && !input.modifiers.command;
    let command_shift = input.modifiers.shift && input.modifiers.command;
    input.events.iter().find_map(|event| match event {
        Event::Copy => Some(Action::Copy),
        Event::Cut if shift_only => Some(Action::RippleDelete),
        Event::Cut if command_shift => Some(Action::ClearInOut),
        Event::Cut => Some(Action::Cut),
        Event::Paste(_) => Some(Action::Paste(PlaceMode::Overwrite)),
        _ => None,
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
        Action::Copy => editor.copy(),
        Action::AddMarker => editor.add_marker(),
        Action::SpeedDuration => editor.speed_duration(),
        Action::ToggleEnabled => editor.toggle_enabled(),
        Action::ToggleLink => editor.toggle_link(),
        Action::PasteAttributes => editor.paste_attributes(),
        Action::NewTitle => {
            placing::new_generated(editor, Generator::Title(Title::default()));
        }
        Action::JumpToMarker { forward } => editor.jump_to_marker(forward),
        Action::ClearMarker => {
            if let Some(id) = editor.marker_at_playhead() {
                editor.clear_marker(id);
            }
        }
        Action::Cut => editor.cut(),
        Action::Paste(mode) => editor.paste(mode),
        Action::AddEdit => editor.add_edit(),
        Action::Transition(kind) => crate::effects::apply_default_transition(editor, kind),
        Action::RippleDelete => editor.ripple_delete(),
        Action::Lift => editor.lift(),
        Action::MarkIn | Action::MarkOut | Action::ClearInOut => mark(editor, action),
        Action::GoToMark(edge) => editor.go_to_mark(edge),
        Action::RippleTrimToPlayhead(edge) => editor.ripple_trim_to_playhead(edge),
        Action::MatchFrame => editor.match_frame(),
        Action::LiftMarked => editor.lift_marked(),
        Action::ExtractMarked => editor.extract_marked(),
        Action::Place(mode) => placing::edit_from_source(editor, mode),
        Action::Tool(tool) => editor.view.tool = tool,
        Action::ToggleSnapping => editor.view.snapping = !editor.view.snapping,
        Action::ZoomIn => editor.view.zoom(2.0, None),
        Action::ZoomOut => editor.view.zoom(0.5, None),
        Action::ZoomToFit => editor.view.zoom_to_fit(editor.timeline.sequence()),
        Action::CancelDrag => editor.view.cancel_drag(),
        Action::ClipGain => editor.prompt_clip_gain(),
        Action::ToggleMulticamView => editor.toggle_multicam_view(),
    }
}

/// Marks the focused monitor: the source, or the sequence the program shows.
fn mark(editor: &mut Editor, action: Action) {
    match (editor.focus, &mut editor.source) {
        (Focus::Source, Some(source)) => match action {
            Action::MarkIn => source.mark_in(),
            Action::MarkOut => source.mark_out(),
            _ => source.marks = Marks::default(),
        },
        (Focus::Source, None) => {}
        (Focus::Program, _) => match action {
            Action::MarkIn => editor.mark_sequence_in(),
            Action::MarkOut => editor.mark_sequence_out(),
            _ => editor.clear_sequence_marks(),
        },
    }
}
