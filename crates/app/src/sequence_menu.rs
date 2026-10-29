use egui::Button;
use timeline::{Edge, TrackKind};

use crate::editor::Editor;
use crate::keymap::Command;
use crate::monitors::Focus;
use crate::project_files::PathPurpose;

pub(crate) fn sequence_menu(editor: &mut Editor, ui: &mut egui::Ui) {
    if ui.button("Sequence Settings…").clicked() {
        editor.open_sequence_settings();
    }
    let sequence = editor.timeline.sequence();
    let has_clips = sequence.tracks().any(|(_, track)| !track.clips.is_empty());
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
    if ui.button("Delete Render Files").clicked() {
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
    ui.checkbox(
        &mut editor.clip_art.show_thumbnails,
        "Show Video Thumbnails",
    );
    ui.checkbox(&mut editor.clip_art.show_waveforms, "Show Audio Waveforms");
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
