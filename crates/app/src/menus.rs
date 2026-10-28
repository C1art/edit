use egui::{Button, Key, MenuBar, ViewportCommand};
use timeline::{Gain, PlaceMode, SetClipGain, TrackKind};

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
        });
        ui.menu_button("Markers", |ui| {
            crate::marker_dialog::menu(editor, ui);
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

pub fn gain_dialog(editor: &mut Editor, ctx: &egui::Context) {
    let Some(db) = &mut editor.gain_prompt else {
        return;
    };
    let mut apply = false;
    let mut cancelled = false;
    egui::Window::new("Audio Gain")
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Set gain to");
                ui.add(
                    egui::DragValue::new(db)
                        .range(Gain::SILENT.db()..=Gain::MAX.db())
                        .speed(0.1)
                        .suffix(" dB"),
                );
            });
            ui.horizontal(|ui| {
                apply =
                    ui.button("OK").clicked() || ui.input(|input| input.key_pressed(Key::Enter));
                cancelled = ui.button("Cancel").clicked()
                    || ui.input(|input| input.key_pressed(Key::Escape));
            });
        });
    if apply {
        let gain = Gain::from_db(*db);
        editor.gain_prompt = None;
        let clips = editor.view.selection.clone();
        editor.apply_levels(SetClipGain { clips, gain });
    } else if cancelled {
        editor.gain_prompt = None;
    }
}
