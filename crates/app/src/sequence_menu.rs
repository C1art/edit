use egui::Button;
use timeline::{Edge, TrackKind};

use crate::editor::Editor;
use crate::gap_navigation::GapScope;
use crate::keymap::Command;
use crate::monitors::Focus;
use crate::project_files::PathPurpose;

pub(crate) fn sequence_menu(editor: &mut Editor, ui: &mut egui::Ui) {
    if ui.button("Sequence Settings…").clicked() {
        editor.open_sequence_settings();
    }
    let sequence = editor.timeline.sequence();
    let has_clips = sequence.has_clips();
    let marked = sequence.marks.is_set();
    let reframe = Button::new("Auto Reframe Sequence…");
    if ui.add_enabled(has_clips, reframe).clicked() {
        editor.open_reframe_dialog();
    }
    let selected = !editor.view.selection.is_empty();
    if ui
        .add_enabled(selected || marked, Button::new("Make Subsequence"))
        .on_hover_text("A new sequence of the selected clips, or of In to Out")
        .on_disabled_hover_text("Select clips or mark In and Out first")
        .clicked()
    {
        editor.make_subsequence();
    }
    if ui
        .add_enabled(has_clips, Button::new("Close Gap"))
        .on_hover_text("Remove every stretch of time no track uses")
        .clicked()
    {
        editor.close_all_gaps();
    }
    ui.add_enabled_ui(has_clips, |ui| {
        ui.menu_button("Go to Gap", |ui| go_to_gap(editor, ui))
    });
    ui.separator();
    let add_edit = Button::new("Add Edit").shortcut_text(editor.keymap.label(Command::AddEdit));
    if ui.add_enabled(has_clips, add_edit).clicked() {
        editor.add_edit();
    }
    let add_edit_all = Button::new("Add Edit to All Tracks")
        .shortcut_text(editor.keymap.label(Command::AddEditToAllTracks));
    if ui.add_enabled(has_clips, add_edit_all).clicked() {
        editor.add_edit_to_all_tracks();
    }
    let render =
        Button::new("Render In to Out").shortcut_text(editor.keymap.label(Command::RenderInToOut));
    if ui.add_enabled(has_clips, render).clicked() {
        editor.render_in_to_out();
    }
    let delete = Button::new("Delete Render Files");
    if ui.add_enabled(editor.has_render_files(), delete).clicked() {
        editor.delete_render_files();
    }
    ui.separator();
    let transition = Button::new("Apply Video Transition")
        .shortcut_text(editor.keymap.label(Command::VideoTransition));
    if ui.add_enabled(has_clips, transition).clicked() {
        crate::effects::apply_default_transition(editor, TrackKind::Video);
    }
    let crossfade = Button::new("Apply Audio Transition")
        .shortcut_text(editor.keymap.label(Command::AudioTransition));
    if ui.add_enabled(has_clips, crossfade).clicked() {
        crate::effects::apply_default_transition(editor, TrackKind::Audio);
    }
    let removable = !editor.view.selection.is_empty() || editor.view.gap.is_some();
    let lift = Button::new("Lift").shortcut_text(editor.keymap.label(Command::Clear));
    if ui.add_enabled(removable, lift).clicked() {
        editor.lift();
    }
    let ripple_delete =
        Button::new("Ripple Delete").shortcut_text(editor.keymap.label(Command::RippleDelete));
    if ui.add_enabled(removable, ripple_delete).clicked() {
        editor.ripple_delete();
    }
    ui.separator();
    // Both measure the program first; normalizing is then one click in the window.
    let measuring = ui
        .add_enabled(has_clips, Button::new("Measure Loudness…"))
        .clicked();
    let normalize = Button::new("Normalize Master Track…");
    if ui.add_enabled(has_clips, normalize).clicked() || measuring {
        crate::loudness_window::measure(editor);
    }
    ui.separator();
    if ui.button("Import Captions…").clicked() {
        editor.prompt_path(PathPurpose::ImportCaptions);
    }
    let has_captions = !editor.timeline.sequence().captions.items.is_empty();
    if ui
        .add_enabled(has_captions, Button::new("Export Captions…"))
        .clicked()
    {
        editor.prompt_path(PathPurpose::ExportCaptions);
    }
    ui.separator();
    let lift =
        Button::new("Lift In to Out").shortcut_text(editor.keymap.label(Command::LiftMarked));
    if ui.add_enabled(marked, lift).clicked() {
        editor.lift_marked();
    }
    let extract =
        Button::new("Extract In to Out").shortcut_text(editor.keymap.label(Command::ExtractMarked));
    if ui.add_enabled(marked, extract).clicked() {
        editor.extract_marked();
    }
    let previous = Button::new("Ripple Trim Previous Edit to Playhead")
        .shortcut_text(editor.keymap.label(Command::RippleTrimPrevious));
    if ui.add_enabled(has_clips, previous).clicked() {
        editor.ripple_trim_to_playhead(Edge::Head);
    }
    let next = Button::new("Ripple Trim Next Edit to Playhead")
        .shortcut_text(editor.keymap.label(Command::RippleTrimNext));
    if ui.add_enabled(has_clips, next).clicked() {
        editor.ripple_trim_to_playhead(Edge::Tail);
    }
    for (edge, name, command) in [
        (
            Edge::Head,
            "Trim Previous Edit to Playhead",
            Command::TrimPrevious,
        ),
        (Edge::Tail, "Trim Next Edit to Playhead", Command::TrimNext),
    ] {
        let trim = Button::new(name).shortcut_text(editor.keymap.label(command));
        if ui.add_enabled(has_clips, trim).clicked() {
            editor.trim_to_playhead(edge);
        }
    }
    let match_frame =
        Button::new("Match Frame").shortcut_text(editor.keymap.label(Command::MatchFrame));
    if ui.add_enabled(has_clips, match_frame).clicked() {
        editor.match_frame();
    }
    let reverse_match = Button::new("Reverse Match Frame")
        .shortcut_text(editor.keymap.label(Command::ReverseMatchFrame));
    if ui.add_enabled(has_clips, reverse_match).clicked() {
        editor.reverse_match_frame();
    }
    ui.separator();
    if ui.button("Add Video Track").clicked() {
        editor.add_track(TrackKind::Video);
    }
    if ui.button("Add Audio Track").clicked() {
        editor.add_track(TrackKind::Audio);
    }
    if ui.button("Simplify Sequence…").clicked() {
        editor.open_simplify();
    }
    ui.menu_button("Delete Empty Tracks", |ui| {
        let choices = [
            ("Video and Audio", &[TrackKind::Video, TrackKind::Audio][..]),
            ("Video Only", &[TrackKind::Video]),
            ("Audio Only", &[TrackKind::Audio]),
        ];
        for (label, kinds) in choices {
            if ui.button(label).clicked() {
                editor.delete_empty_tracks(kinds);
            }
        }
    });
    if ui.button("Record Voiceover…").clicked() {
        editor.open_voiceover();
    }
    ui.separator();
    ui.checkbox(&mut editor.use_proxies, "Use Proxies")
        .on_hover_text("Play proxies where media has them; export uses the originals");
    ui.checkbox(&mut editor.scrub_audio, "Audio During Scrubbing")
        .on_hover_text(editor.keymap.tip(
            "Hear a moment of sound wherever dragging the playhead or stepping lands",
            Command::ToggleAudioScrubbing,
        ));
    ui.checkbox(
        &mut editor.selection_follows_playhead,
        "Selection Follows Playhead",
    )
    .on_hover_text("Select the clips under the playhead on the targeted tracks as it moves");
}

/// Setting, finding and clearing In and Out on the focused monitor, as their shortcuts
/// do; Premiere keeps them in its Markers menu.
pub(crate) fn in_out_items(editor: &mut Editor, ui: &mut egui::Ui) {
    let marks = match (editor.focus, &editor.source) {
        (Focus::Source, Some(source)) => Some(source.marks),
        (Focus::Source, None) => None,
        (Focus::Program, _) => Some(editor.timeline.sequence().marks),
    };
    let has_in = marks.is_some_and(|marks| marks.mark_in.is_some());
    let has_out = marks.is_some_and(|marks| marks.mark_out.is_some());
    let selected = !editor.view.selection.is_empty();
    let items = [
        (Command::MarkIn, marks.is_some()),
        (Command::MarkOut, marks.is_some()),
        (Command::MarkClip, marks.is_some()),
        (Command::MarkSelection, selected),
        (Command::ClearInOut, has_in || has_out),
        (Command::ClearIn, has_in),
        (Command::ClearOut, has_out),
        (Command::GoToIn, has_in),
        (Command::GoToOut, has_out),
    ];
    for (command, enabled) in items {
        let button = Button::new(command.name()).shortcut_text(editor.keymap.label(command));
        if ui.add_enabled(enabled, button).clicked() {
            crate::shortcuts::run(editor, command);
        }
    }
}

fn go_to_gap(editor: &mut Editor, ui: &mut egui::Ui) {
    let choices = [
        (
            Command::NextGapInSequence,
            "Next in Sequence",
            GapScope::Sequence,
            true,
        ),
        (
            Command::PreviousGapInSequence,
            "Previous in Sequence",
            GapScope::Sequence,
            false,
        ),
        (
            Command::NextGapInTrack,
            "Next in Track",
            GapScope::Track,
            true,
        ),
        (
            Command::PreviousGapInTrack,
            "Previous in Track",
            GapScope::Track,
            false,
        ),
    ];
    for (command, name, scope, forward) in choices {
        let button = Button::new(name).shortcut_text(editor.keymap.label(command));
        if ui.add(button).clicked() {
            editor.go_to_gap(scope, forward);
            ui.close();
        }
    }
}
