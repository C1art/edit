use egui::{Button, MenuBar, ViewportCommand};
use timeline::{Edge, Generator, PlaceMode, Rgb, Title, TrackKind};

use crate::editor::Editor;
use crate::keymap::Command;
use crate::placing;
use crate::project_files::{AfterDiscard, PathPurpose};
use crate::timeline_view::Tool;

pub fn menu_bar(editor: &mut Editor, ui: &mut egui::Ui) {
    MenuBar::new().ui(ui, |ui| {
        ui.menu_button("File", |ui| {
            let new =
                Button::new("New Project").shortcut_text(editor.keymap.label(Command::NewProject));
            if ui.add(new).clicked() {
                editor.discard_then(AfterDiscard::NewProject);
            }
            let open = Button::new("Open Project…")
                .shortcut_text(editor.keymap.label(Command::OpenProject));
            if ui.add(open).clicked() {
                editor.prompt_path(PathPurpose::OpenProject);
            }
            let save = Button::new("Save").shortcut_text(editor.keymap.label(Command::Save));
            if ui.add(save).clicked() {
                editor.save_project();
            }
            let save_as =
                Button::new("Save As…").shortcut_text(editor.keymap.label(Command::SaveAs));
            if ui.add(save_as).clicked() {
                editor.prompt_path(PathPurpose::SaveProject);
            }
            ui.separator();
            let import = Button::new("Import…").shortcut_text(editor.keymap.label(Command::Import));
            if ui.add(import).clicked() {
                editor.prompt_path(PathPurpose::Import);
            }
            ui.menu_button("New", |ui| {
                let title =
                    Button::new("Title").shortcut_text(editor.keymap.label(Command::NewTitle));
                if ui.add(title).clicked() {
                    placing::new_generated(editor, Generator::Title(Title::default()));
                }
                if ui.button("Sequence").clicked() {
                    editor.new_sequence();
                }
                if ui.button("Color Matte").clicked() {
                    let color = Rgb::new(32, 32, 48);
                    placing::new_generated(editor, Generator::ColorMatte { color });
                }
                if ui.button("Adjustment Layer").clicked() {
                    placing::new_generated(editor, Generator::AdjustmentLayer);
                }
            });
            let export = Button::new("Export Media…")
                .shortcut_text(editor.keymap.label(Command::ExportMedia));
            if ui.add(export).clicked() {
                crate::export::open(editor);
            }
            if ui.button("Export Queue…").clicked() {
                editor.export_queue.open = true;
            }
            let frame = Button::new("Export Frame…")
                .shortcut_text(editor.keymap.label(Command::ExportFrame));
            if ui.add(frame).clicked() {
                editor.prompt_path(PathPurpose::ExportFrame);
            }
            if ui.button("Export EDL…").clicked() {
                editor.prompt_path(PathPurpose::ExportEdl);
            }
            ui.separator();
            if ui.button("Quit").clicked() {
                ui.ctx().send_viewport_cmd(ViewportCommand::Close);
            }
        });
        ui.menu_button("Edit", |ui| {
            let undo = editor
                .timeline
                .undo_name()
                .map_or("Undo".to_owned(), |name| format!("Undo {name}"));
            let undo = Button::new(undo).shortcut_text(editor.keymap.label(Command::Undo));
            if ui
                .add_enabled(editor.timeline.undo_name().is_some(), undo)
                .clicked()
            {
                editor.undo();
            }
            let redo = editor
                .timeline
                .redo_name()
                .map_or("Redo".to_owned(), |name| format!("Redo {name}"));
            let redo = Button::new(redo).shortcut_text(editor.keymap.label(Command::Redo));
            if ui
                .add_enabled(editor.timeline.redo_name().is_some(), redo)
                .clicked()
            {
                editor.redo();
            }
            ui.separator();
            let selected = !editor.view.selection.is_empty();
            let cut = Button::new("Cut").shortcut_text(editor.keymap.label(Command::Cut));
            if ui.add_enabled(selected, cut).clicked() {
                editor.cut();
            }
            let copy = Button::new("Copy").shortcut_text(editor.keymap.label(Command::Copy));
            if ui.add_enabled(selected, copy).clicked() {
                editor.copy();
            }
            let can_paste = editor.clipboard.is_some();
            let paste = Button::new("Paste").shortcut_text(editor.keymap.label(Command::Paste));
            if ui.add_enabled(can_paste, paste).clicked() {
                editor.paste(PlaceMode::Overwrite);
            }
            let paste_insert = Button::new("Paste Insert")
                .shortcut_text(editor.keymap.label(Command::PasteInsert));
            if ui.add_enabled(can_paste, paste_insert).clicked() {
                editor.paste(PlaceMode::Insert);
            }
            let paste_attributes = Button::new("Paste Attributes…")
                .shortcut_text(editor.keymap.label(Command::PasteAttributes));
            if ui.add_enabled(can_paste, paste_attributes).clicked() {
                editor.paste_attributes();
            }
            if ui.button("Remove Attributes…").clicked() {
                editor.remove_attributes();
            }
            ui.separator();
            ui.menu_button("Preferences", |ui| {
                let decoding = &mut editor.hardware_decoding;
                let device = media::hardware_decoder().map_or_else(
                    || "No GPU decoder is in use yet.".to_owned(),
                    |name| format!("Decoding on {name}."),
                );
                let hint = format!("Applies to media opened from now on. {device}");
                if ui
                    .checkbox(decoding, "Hardware-Accelerated Decoding")
                    .on_hover_text(hint)
                    .changed()
                {
                    media::set_hardware_decoding(*decoding);
                }
                ui.horizontal(|ui| {
                    let autosave = &mut editor.autosave;
                    ui.checkbox(&mut autosave.enabled, "Auto-Save Every");
                    ui.add_enabled(
                        autosave.enabled,
                        egui::DragValue::new(&mut autosave.minutes)
                            .range(1..=60)
                            .suffix(" min"),
                    );
                });
            });
            let shortcuts = Button::new("Keyboard Shortcuts…")
                .shortcut_text(editor.keymap.label(Command::KeyboardShortcuts));
            if ui.add(shortcuts).clicked() {
                editor.keymap_window = Some(crate::keymap::KeymapWindow::default());
            }
        });
        ui.menu_button("Clip", |ui| {
            if ui
                .add(Button::new("Insert").shortcut_text(editor.keymap.label(Command::Insert)))
                .clicked()
            {
                placing::edit_from_source(editor, PlaceMode::Insert);
            }
            if ui
                .add(
                    Button::new("Overwrite").shortcut_text(editor.keymap.label(Command::Overwrite)),
                )
                .clicked()
            {
                placing::edit_from_source(editor, PlaceMode::Overwrite);
            }
            let subclip = Button::new("Make Subclip")
                .shortcut_text(editor.keymap.label(Command::MakeSubclip));
            if ui.add(subclip).clicked() {
                editor.make_subclip();
            }
            ui.separator();
            crate::clip_menu::items(editor, ui);
        });
        ui.menu_button("Markers", |ui| {
            crate::marker_dialog::menu(editor, ui);
            ui.separator();
            let go_to_in =
                Button::new("Go to In").shortcut_text(editor.keymap.label(Command::GoToIn));
            if ui.add(go_to_in).clicked() {
                editor.go_to_mark(Edge::Head);
            }
            let go_to_out =
                Button::new("Go to Out").shortcut_text(editor.keymap.label(Command::GoToOut));
            if ui.add(go_to_out).clicked() {
                editor.go_to_mark(Edge::Tail);
            }
        });
        ui.menu_button("Sequence", |ui| {
            if ui.button("Sequence Settings…").clicked() {
                editor.open_sequence_settings();
            }
            ui.separator();
            let add_edit =
                Button::new("Add Edit").shortcut_text(editor.keymap.label(Command::AddEdit));
            if ui.add(add_edit).clicked() {
                editor.add_edit();
            }
            let render = Button::new("Render In to Out")
                .shortcut_text(editor.keymap.label(Command::RenderInToOut));
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
                .add(
                    Button::new("Ripple Delete")
                        .shortcut_text(editor.keymap.label(Command::RippleDelete)),
                )
                .clicked()
            {
                editor.ripple_delete();
            }
            ui.separator();
            if ui.button("Measure Loudness…").clicked() {
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
                let button =
                    Button::new(command.name()).shortcut_text(editor.keymap.label(command));
                if ui.add(button).clicked() {
                    run(editor);
                }
            }
            let clear = Button::new("Clear In and Out")
                .shortcut_text(editor.keymap.label(Command::ClearInOut));
            if ui.add(clear).clicked() {
                editor.clear_sequence_marks();
            }
            if ui
                .add(
                    Button::new("Lift In to Out")
                        .shortcut_text(editor.keymap.label(Command::LiftMarked)),
                )
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
            if ui
                .add(
                    Button::new("Match Frame")
                        .shortcut_text(editor.keymap.label(Command::MatchFrame)),
                )
                .clicked()
            {
                editor.match_frame();
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
        });
    });
}

pub fn timeline_toolbar(editor: &mut Editor, ui: &mut egui::Ui) {
    crate::nesting::breadcrumbs(editor, ui);
    ui.horizontal(|ui| {
        for tool in Tool::ALL {
            if ui
                .selectable_label(editor.view.tool == tool, tool.label())
                .on_hover_text(tool.hint())
                .clicked()
            {
                editor.view.tool = tool;
            }
        }
        ui.separator();
        ui.toggle_value(&mut editor.view.snapping, "Snap")
            .on_hover_text("Snap to clip edges and the playhead (S)");
        ui.separator();
        if ui.button("−").on_hover_text("Zoom out (-)").clicked() {
            editor.view.zoom(0.5, None);
        }
        if ui.button("+").on_hover_text("Zoom in (=)").clicked() {
            editor.view.zoom(2.0, None);
        }
        if ui
            .button("Fit")
            .on_hover_text("Zoom to sequence (\\)")
            .clicked()
        {
            editor.view.zoom_to_fit(editor.timeline.sequence());
        }
        ui.separator();
        if ui.button("+ Video").clicked() {
            editor.add_track(TrackKind::Video);
        }
        if ui.button("+ Audio").clicked() {
            editor.add_track(TrackKind::Audio);
        }
    });
}
