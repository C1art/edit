use egui::{Key, Modifiers};
use timeline::{Edge, Generator, Marks, PlaceMode, Title, TrackKind};

use crate::editor::Editor;
use crate::keymap::{take_press, Command, KeymapWindow};
use crate::monitors::Focus;
use crate::placing;
use crate::project_files::{AfterDiscard, PathPurpose};
use crate::transport;

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

/// Editor-wide keys. Skipped while a text field has focus so typing isn't hijacked, and
/// while a key is being recorded for a shortcut.
pub fn handle(editor: &mut Editor, ctx: &egui::Context) {
    let recording = editor
        .keymap_window
        .as_ref()
        .is_some_and(|window| window.recording.is_some());
    if ctx.egui_wants_keyboard_input() || recording {
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
    while let Some(command) = take_press(ctx, |chord| editor.keymap.command(chord)) {
        run(editor, command);
    }
}

pub(crate) fn run(editor: &mut Editor, command: Command) {
    if let Some(key) = command.transport() {
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
        return;
    }
    if let Some(tool) = command.tool() {
        editor.view.tool = tool;
        return;
    }
    match command {
        Command::NewProject => editor.discard_then(AfterDiscard::NewProject),
        Command::OpenProject => editor.prompt_path(PathPurpose::OpenProject),
        Command::Save => editor.save_project(),
        Command::SaveAs => editor.prompt_path(PathPurpose::SaveProject),
        Command::Import => editor.prompt_path(PathPurpose::Import),
        Command::ExportMedia => crate::export::open(editor),
        Command::ExportFrame => editor.prompt_path(PathPurpose::ExportFrame),
        Command::NewTitle => {
            placing::new_generated(editor, Generator::Title(Title::default()));
        }
        Command::Undo => editor.undo(),
        Command::Redo => editor.redo(),
        Command::Cut => editor.cut(),
        Command::Copy => editor.copy(),
        Command::Paste => editor.paste(PlaceMode::Overwrite),
        Command::PasteInsert => editor.paste(PlaceMode::Insert),
        Command::PasteAttributes => editor.paste_attributes(),
        Command::Clear => editor.lift(),
        Command::RippleDelete => editor.ripple_delete(),
        Command::CancelDrag => editor.view.cancel_drag(),
        Command::SelectAll => editor.select_all(),
        Command::DeselectAll => editor.view.selection.clear(),
        Command::KeyboardShortcuts => {
            editor.keymap_window = Some(KeymapWindow::default());
        }
        Command::SpeedDuration => editor.speed_duration(),
        Command::AudioGain => editor.prompt_clip_gain(),
        Command::ToggleEnabled => editor.toggle_enabled(),
        Command::ToggleLink => editor.toggle_link(),
        Command::Group => editor.set_grouped(true),
        Command::Ungroup => editor.set_grouped(false),
        Command::MakeSubclip => editor.make_subclip(),
        Command::NudgeLeft => editor.nudge(-1),
        Command::NudgeRight => editor.nudge(1),
        Command::NudgeLeftFive => editor.nudge(-5),
        Command::NudgeRightFive => editor.nudge(5),
        Command::NudgeUp => editor.nudge_track(true),
        Command::NudgeDown => editor.nudge_track(false),
        Command::Insert => placing::edit_from_source(editor, PlaceMode::Insert),
        Command::Overwrite => placing::edit_from_source(editor, PlaceMode::Overwrite),
        Command::AddEdit => editor.add_edit(),
        Command::ExtendEdit => editor.extend_edit(),
        Command::SelectAtPlayhead => editor.select_at_playhead(),
        Command::VideoTransition => {
            crate::effects::apply_default_transition(editor, TrackKind::Video);
        }
        Command::AudioTransition => {
            crate::effects::apply_default_transition(editor, TrackKind::Audio);
        }
        Command::LiftMarked => editor.lift_marked(),
        Command::ExtractMarked => editor.extract_marked(),
        Command::RippleTrimPrevious => editor.ripple_trim_to_playhead(Edge::Head),
        Command::RippleTrimNext => editor.ripple_trim_to_playhead(Edge::Tail),
        Command::MatchFrame => editor.match_frame(),
        Command::ToggleSnapping => editor.view.snapping = !editor.view.snapping,
        Command::ZoomIn => editor.view.zoom(2.0, None),
        Command::ZoomOut => editor.view.zoom(0.5, None),
        Command::ZoomToFit => editor.view.zoom_to_fit(editor.timeline.sequence()),
        Command::IncreaseVideoHeights | Command::DecreaseVideoHeights => {
            let taller = command == Command::IncreaseVideoHeights;
            let sequence = editor.timeline.sequence();
            editor.view.heights.step(sequence, TrackKind::Video, taller);
        }
        Command::IncreaseAudioHeights | Command::DecreaseAudioHeights => {
            let taller = command == Command::IncreaseAudioHeights;
            let sequence = editor.timeline.sequence();
            editor.view.heights.step(sequence, TrackKind::Audio, taller);
        }
        Command::ExpandAllTracks | Command::MinimizeAllTracks => {
            let expanded = command == Command::ExpandAllTracks;
            editor
                .view
                .heights
                .all(editor.timeline.sequence(), expanded);
        }
        Command::MulticamView => editor.toggle_multicam_view(),
        Command::RenderInToOut => editor.render_in_to_out(),
        Command::DeleteRenderFiles => editor.delete_render_files(),
        Command::MarkIn | Command::MarkOut | Command::ClearInOut => mark(editor, command),
        Command::MarkClip => editor.mark_clip(),
        Command::MarkSelection => editor.mark_selection(),
        Command::GoToIn => editor.go_to_mark(Edge::Head),
        Command::GoToOut => editor.go_to_mark(Edge::Tail),
        Command::AddMarker => editor.add_marker(),
        Command::NextMarker => editor.jump_to_marker(true),
        Command::PreviousMarker => editor.jump_to_marker(false),
        Command::ClearMarker => {
            if let Some(id) = editor.marker_at_playhead() {
                editor.clear_marker(id);
            }
        }
        // Tools and playback were dispatched above.
        _ => {}
    }
}

/// Marks the focused monitor: the source, or the sequence the program shows.
fn mark(editor: &mut Editor, command: Command) {
    match (editor.focus, &mut editor.source) {
        (Focus::Source, Some(source)) => match command {
            Command::MarkIn => source.mark_in(),
            Command::MarkOut => source.mark_out(),
            _ => source.marks = Marks::default(),
        },
        (Focus::Source, None) => {}
        (Focus::Program, _) => match command {
            Command::MarkIn => editor.mark_sequence_in(),
            Command::MarkOut => editor.mark_sequence_out(),
            _ => editor.clear_sequence_marks(),
        },
    }
}
