use egui::{Button, MenuBar, ViewportCommand};
use timeline::{Edge, Generator, PlaceMode, Rgb, Title, TrackKind};

use crate::editor::Editor;
use crate::placing;
use crate::project_files::{AfterDiscard, PathPurpose};
use crate::shortcuts;
use crate::timeline_view::Tool;

pub fn menu_bar(editor: &mut Editor, ui: &mut egui::Ui) {
    MenuBar::new().ui(ui, |ui| {
        let shortcut = |ui: &egui::Ui, shortcut| ui.ctx().format_shortcut(&shortcut);
        ui.menu_button("File", |ui| {
            let new = Button::new("New Project").shortcut_text(shortcut(ui, shortcuts::NEW));
            if ui.add(new).clicked() {
                editor.discard_then(AfterDiscard::NewProject);
            }
            let open = Button::new("Open Project…").shortcut_text(shortcut(ui, shortcuts::OPEN));
            if ui.add(open).clicked() {
                editor.prompt_path(PathPurpose::OpenProject);
            }
            let save = Button::new("Save").shortcut_text(shortcut(ui, shortcuts::SAVE));
            if ui.add(save).clicked() {
                editor.save_project();
            }
            let save_as = Button::new("Save As…").shortcut_text(shortcut(ui, shortcuts::SAVE_AS));
            if ui.add(save_as).clicked() {
                editor.prompt_path(PathPurpose::SaveProject);
            }
            ui.separator();
            let import = Button::new("Import…").shortcut_text(shortcut(ui, shortcuts::IMPORT));
            if ui.add(import).clicked() {
                editor.prompt_path(PathPurpose::Import);
            }
            ui.menu_button("New", |ui| {
                let title = Button::new("Title").shortcut_text(shortcut(ui, shortcuts::NEW_TITLE));
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
            let export =
                Button::new("Export Media…").shortcut_text(shortcut(ui, shortcuts::EXPORT));
            if ui.add(export).clicked() {
                crate::export::open(editor);
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
            let undo = Button::new(undo).shortcut_text(shortcut(ui, shortcuts::UNDO));
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
            let redo = Button::new(redo).shortcut_text(shortcut(ui, shortcuts::REDO));
            if ui
                .add_enabled(editor.timeline.redo_name().is_some(), redo)
                .clicked()
            {
                editor.redo();
            }
            ui.separator();
            let selected = !editor.view.selection.is_empty();
            let cut = Button::new("Cut").shortcut_text(shortcut(ui, shortcuts::CUT));
            if ui.add_enabled(selected, cut).clicked() {
                editor.cut();
            }
            let copy = Button::new("Copy").shortcut_text(shortcut(ui, shortcuts::COPY));
            if ui.add_enabled(selected, copy).clicked() {
                editor.copy();
            }
            let can_paste = editor.clipboard.is_some();
            let paste = Button::new("Paste").shortcut_text(shortcut(ui, shortcuts::PASTE));
            if ui.add_enabled(can_paste, paste).clicked() {
                editor.paste(PlaceMode::Overwrite);
            }
            let paste_insert =
                Button::new("Paste Insert").shortcut_text(shortcut(ui, shortcuts::PASTE_INSERT));
            if ui.add_enabled(can_paste, paste_insert).clicked() {
                editor.paste(PlaceMode::Insert);
            }
            let paste_attributes = Button::new("Paste Attributes…")
                .shortcut_text(shortcut(ui, shortcuts::PASTE_ATTRIBUTES));
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
        });
        ui.menu_button("Clip", |ui| {
            if ui.add(Button::new("Insert").shortcut_text(",")).clicked() {
                placing::edit_from_source(editor, PlaceMode::Insert);
            }
            if ui
                .add(Button::new("Overwrite").shortcut_text("."))
                .clicked()
            {
                placing::edit_from_source(editor, PlaceMode::Overwrite);
            }
            ui.separator();
            let speed = Button::new("Speed/Duration…")
                .shortcut_text(shortcut(ui, shortcuts::SPEED_DURATION));
            if ui.add(speed).clicked() {
                editor.speed_duration();
            }
            let enable =
                Button::new("Enable").shortcut_text(shortcut(ui, shortcuts::TOGGLE_ENABLED));
            if ui.add(enable).clicked() {
                editor.toggle_enabled();
            }
            let link = if editor.selection_linked() == Some(true) {
                "Unlink"
            } else {
                "Link"
            };
            let link = Button::new(link).shortcut_text(shortcut(ui, shortcuts::TOGGLE_LINK));
            if ui.add(link).clicked() {
                editor.toggle_link();
            }
            let gain = Button::new("Audio Gain…").shortcut_text("G");
            if ui.add(gain).clicked() {
                editor.prompt_clip_gain();
            }
            if ui.button("Add Frame Hold").clicked() {
                editor.add_frame_hold();
            }
            ui.separator();
            if ui.button("Nest").clicked() {
                editor.nest_selection();
            }
        });
        ui.menu_button("Markers", |ui| {
            crate::marker_dialog::menu(editor, ui);
            ui.separator();
            let go_to_in = Button::new("Go to In").shortcut_text(shortcut(ui, shortcuts::GO_TO_IN));
            if ui.add(go_to_in).clicked() {
                editor.go_to_mark(Edge::Head);
            }
            let go_to_out =
                Button::new("Go to Out").shortcut_text(shortcut(ui, shortcuts::GO_TO_OUT));
            if ui.add(go_to_out).clicked() {
                editor.go_to_mark(Edge::Tail);
            }
        });
        ui.menu_button("Sequence", |ui| {
            let add_edit = Button::new("Add Edit").shortcut_text(shortcut(ui, shortcuts::ADD_EDIT));
            if ui.add(add_edit).clicked() {
                editor.add_edit();
            }
            let transition = Button::new("Apply Video Transition")
                .shortcut_text(shortcut(ui, shortcuts::TRANSITION));
            if ui.add(transition).clicked() {
                crate::effects::apply_default_transition(editor, TrackKind::Video);
            }
            let crossfade = Button::new("Apply Audio Transition")
                .shortcut_text(shortcut(ui, shortcuts::AUDIO_TRANSITION));
            if ui.add(crossfade).clicked() {
                crate::effects::apply_default_transition(editor, TrackKind::Audio);
            }
            if ui
                .add(Button::new("Lift").shortcut_text("Delete"))
                .clicked()
            {
                editor.lift();
            }
            if ui
                .add(Button::new("Ripple Delete").shortcut_text("Shift+Delete"))
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
            if ui.add(Button::new("Mark In").shortcut_text("I")).clicked() {
                editor.mark_sequence_in();
            }
            if ui.add(Button::new("Mark Out").shortcut_text("O")).clicked() {
                editor.mark_sequence_out();
            }
            let clear = Button::new("Clear In and Out")
                .shortcut_text(shortcut(ui, shortcuts::CLEAR_IN_OUT));
            if ui.add(clear).clicked() {
                editor.clear_sequence_marks();
            }
            if ui
                .add(Button::new("Lift In to Out").shortcut_text(";"))
                .clicked()
            {
                editor.lift_marked();
            }
            if ui
                .add(Button::new("Extract In to Out").shortcut_text("'"))
                .clicked()
            {
                editor.extract_marked();
            }
            let previous = Button::new("Ripple Trim Previous Edit to Playhead").shortcut_text("Q");
            if ui.add(previous).clicked() {
                editor.ripple_trim_to_playhead(Edge::Head);
            }
            let next = Button::new("Ripple Trim Next Edit to Playhead").shortcut_text("W");
            if ui.add(next).clicked() {
                editor.ripple_trim_to_playhead(Edge::Tail);
            }
            if ui
                .add(Button::new("Match Frame").shortcut_text("F"))
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
