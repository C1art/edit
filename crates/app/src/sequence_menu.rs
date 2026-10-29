use egui::Button;
use timeline::{Edge, TrackKind};

use crate::editor::Editor;
use crate::keymap::Command;
use crate::project_files::PathPurpose;

pub(crate) fn sequence_menu(editor: &mut Editor, ui: &mut egui::Ui) {
    if ui.button("Sequence Settings…").clicked() {
        editor.open_sequence_settings();
    }
    if ui.button("Auto Reframe Sequence…").clicked() {
        editor.open_reframe_dialog();
    }
    if ui
        .button("Make Subsequence")
        .on_hover_text("A new sequence of the selected clips, or of In to Out")
        .clicked()
    {
        editor.make_subsequence();
    }
    if ui
        .button("Close Gap")
        .on_hover_text("Remove every stretch of time no track uses")
        .clicked()
    {
        editor.close_all_gaps();
    }
    ui.separator();
    let add_edit = Button::new("Add Edit").shortcut_text(editor.keymap.label(Command::AddEdit));
    if ui.add(add_edit).clicked() {
        editor.add_edit();
    }
    let add_edit_all = Button::new("Add Edit to All Tracks")
        .shortcut_text(editor.keymap.label(Command::AddEditToAllTracks));
    if ui.add(add_edit_all).clicked() {
        editor.add_edit_to_all_tracks();
    }
    let render =
        Button::new("Render In to Out").shortcut_text(editor.keymap.label(Command::RenderInToOut));
    if ui.add(render).clicked() {
        editor.render_in_to_out();
    }
    if ui.button("Delete Render Files").clicked() {
        editor.delete_render_files();
    }
    ui.separator();
    let transition = Button::new("Apply Video Transition")
        .shortcut_text(editor.keymap.label(Command::VideoTransition));
    if ui.add(transition).clicked() {
        crate::effects::apply_default_transition(editor, TrackKind::Video);
    }
    let crossfade = Button::new("Apply Audio Transition")
        .shortcut_text(editor.keymap.label(Command::AudioTransition));
    if ui.add(crossfade).clicked() {
        crate::effects::apply_default_transition(editor, TrackKind::Audio);
    }
    if ui
        .add(Button::new("Lift").shortcut_text(editor.keymap.label(Command::Clear)))
        .clicked()
    {
        editor.lift();
    }
    if ui
        .add(Button::new("Ripple Delete").shortcut_text(editor.keymap.label(Command::RippleDelete)))
        .clicked()
    {
        editor.ripple_delete();
    }
    ui.separator();
    // Both measure the program first; normalizing is then one click in the window.
    let measuring = ui.button("Measure Loudness…").clicked();
    if ui.button("Normalize Master Track…").clicked() || measuring {
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
    if ui
        .add(Button::new("Mark In").shortcut_text(editor.keymap.label(Command::MarkIn)))
        .clicked()
    {
        editor.mark_sequence_in();
    }
    if ui
        .add(Button::new("Mark Out").shortcut_text(editor.keymap.label(Command::MarkOut)))
        .clicked()
    {
        editor.mark_sequence_out();
    }
    for (command, run) in [
        (Command::MarkClip, Editor::mark_clip as fn(&mut Editor)),
        (Command::MarkSelection, Editor::mark_selection),
    ] {
        let button = Button::new(command.name()).shortcut_text(editor.keymap.label(command));
        if ui.add(button).clicked() {
            run(editor);
        }
    }
    for (command, clear) in [
        (
            Command::ClearInOut,
            Editor::clear_sequence_marks as fn(&mut Editor),
        ),
        (Command::ClearIn, Editor::clear_sequence_in),
        (Command::ClearOut, Editor::clear_sequence_out),
    ] {
        let button = Button::new(command.name()).shortcut_text(editor.keymap.label(command));
        if ui.add(button).clicked() {
            clear(editor);
        }
    }
    if ui
        .add(Button::new("Lift In to Out").shortcut_text(editor.keymap.label(Command::LiftMarked)))
        .clicked()
    {
        editor.lift_marked();
    }
    if ui
        .add(
            Button::new("Extract In to Out")
                .shortcut_text(editor.keymap.label(Command::ExtractMarked)),
        )
        .clicked()
    {
        editor.extract_marked();
    }
    let previous = Button::new("Ripple Trim Previous Edit to Playhead")
        .shortcut_text(editor.keymap.label(Command::RippleTrimPrevious));
    if ui.add(previous).clicked() {
        editor.ripple_trim_to_playhead(Edge::Head);
    }
    let next = Button::new("Ripple Trim Next Edit to Playhead")
        .shortcut_text(editor.keymap.label(Command::RippleTrimNext));
    if ui.add(next).clicked() {
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
        if ui.add(trim).clicked() {
            editor.trim_to_playhead(edge);
        }
    }
    if ui
        .add(Button::new("Match Frame").shortcut_text(editor.keymap.label(Command::MatchFrame)))
        .clicked()
    {
        editor.match_frame();
    }
    if ui
        .add(
            Button::new("Reverse Match Frame")
                .shortcut_text(editor.keymap.label(Command::ReverseMatchFrame)),
        )
        .clicked()
    {
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
    ui.checkbox(
        &mut editor.clip_art.show_thumbnails,
        "Show Video Thumbnails",
    );
    ui.checkbox(&mut editor.clip_art.show_waveforms, "Show Audio Waveforms");
}
