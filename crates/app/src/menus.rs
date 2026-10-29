use egui::{Button, MenuBar, ViewportCommand};
use timeline::{
    Generator, LinkedSelection, PlaceMode, Rgb, Shape, ShapeKind, Ticks, Title, TrackKind,
};

use crate::editor::Editor;
use crate::icons::{self, Icon, Look};
use crate::keymap::{Command, Keymap};
use crate::placing;
use crate::project_files::{AfterDiscard, Interchange, PathPurpose};
use crate::theme;
use crate::timeline_view::Tool;

pub fn menu_bar(editor: &mut Editor, ui: &mut egui::Ui) {
    MenuBar::new().ui(ui, |ui| {
        crate::theme::menu(ui, "File", |ui| {
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
            crate::recent_projects::menu(editor, ui);
            let save = Button::new("Save").shortcut_text(editor.keymap.label(Command::Save));
            if ui.add_enabled(editor.is_dirty(), save).clicked() {
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
                ui.separator();
                for synthetic in [
                    Generator::BarsAndTone,
                    Generator::BlackVideo,
                    Generator::TransparentVideo,
                    Generator::CountingLeader(Default::default()),
                ] {
                    if ui.button(synthetic.kind_name()).clicked() {
                        placing::new_generated(editor, synthetic);
                    }
                }
                ui.separator();
                for kind in ShapeKind::ALL {
                    if ui.button(kind.name()).clicked() {
                        placing::new_generated(editor, Generator::Shape(Shape::new(kind)));
                    }
                }
            });
            let has_program = editor.timeline.sequence().duration() > Ticks::ZERO;
            let export = Button::new("Export Media…")
                .shortcut_text(editor.keymap.label(Command::ExportMedia));
            if ui.add_enabled(has_program, export).clicked() {
                crate::export::open(editor);
            }
            if ui.button("Export Queue…").clicked() {
                editor.export_queue.open = true;
            }
            let frame = Button::new("Export Frame…")
                .shortcut_text(editor.keymap.label(Command::ExportFrame));
            if ui.add_enabled(has_program, frame).clicked() {
                editor.prompt_path(PathPurpose::ExportFrame);
            }
            if ui
                .add_enabled(has_program, Button::new("Export EDL…"))
                .clicked()
            {
                editor.prompt_path(PathPurpose::Export(Interchange::Edl));
            }
            if ui
                .add_enabled(has_program, Button::new("Export Final Cut Pro XML…"))
                .clicked()
            {
                editor.prompt_path(PathPurpose::Export(Interchange::FinalCutXml));
            }
            ui.separator();
            let has_media = !editor.timeline.sequence().media.is_empty();
            let manager = Button::new("Project Manager…");
            if ui.add_enabled(has_media, manager).clicked() {
                editor.open_project_manager();
            }
            ui.separator();
            let quit = Button::new("Quit").shortcut_text(editor.keymap.label(Command::Quit));
            if ui.add(quit).clicked() {
                ui.ctx().send_viewport_cmd(ViewportCommand::Close);
            }
        });
        crate::theme::menu(ui, "Edit", |ui| {
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
            if ui
                .add_enabled(can_paste && selected, paste_attributes)
                .clicked()
            {
                editor.paste_attributes();
            }
            let remove_attributes = Button::new("Remove Attributes…");
            if ui.add_enabled(selected, remove_attributes).clicked() {
                editor.remove_attributes();
            }
            ui.separator();
            let removable = selected || editor.view.gap.is_some();
            let clear = Button::new("Clear").shortcut_text(editor.keymap.label(Command::Clear));
            if ui.add_enabled(removable, clear).clicked() {
                editor.lift();
            }
            let ripple_delete = Button::new("Ripple Delete")
                .shortcut_text(editor.keymap.label(Command::RippleDelete));
            if ui.add_enabled(removable, ripple_delete).clicked() {
                editor.ripple_delete();
            }
            ui.separator();
            let select_all =
                Button::new("Select All").shortcut_text(editor.keymap.label(Command::SelectAll));
            let has_clips = editor.timeline.sequence().has_clips();
            if ui.add_enabled(has_clips, select_all).clicked() {
                editor.select_all();
            }
            let deselect_all = Button::new("Deselect All")
                .shortcut_text(editor.keymap.label(Command::DeselectAll));
            if ui.add_enabled(selected, deselect_all).clicked() {
                editor.deselect_all();
            }
            ui.separator();
            let find = Button::new("Find…").shortcut_text(editor.keymap.label(Command::Find));
            if ui.add(find).clicked() {
                editor.open_find();
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
                ui.menu_button("Audio Output", |ui| {
                    crate::audio_switch::menu(editor, ui);
                });
            });
            let shortcuts = Button::new("Keyboard Shortcuts…")
                .shortcut_text(editor.keymap.label(Command::KeyboardShortcuts));
            if ui.add(shortcuts).clicked() {
                editor.keymap_window = Some(crate::keymap::KeymapWindow::default());
            }
        });
        crate::theme::menu(ui, "Clip", |ui| {
            let has_source = editor.source.is_some();
            let insert = Button::new("Insert").shortcut_text(editor.keymap.label(Command::Insert));
            if ui.add_enabled(has_source, insert).clicked() {
                placing::edit_from_source(editor, PlaceMode::Insert);
            }
            let overwrite =
                Button::new("Overwrite").shortcut_text(editor.keymap.label(Command::Overwrite));
            if ui.add_enabled(has_source, overwrite).clicked() {
                placing::edit_from_source(editor, PlaceMode::Overwrite);
            }
            let subclip = Button::new("Make Subclip")
                .shortcut_text(editor.keymap.label(Command::MakeSubclip));
            if ui.add_enabled(has_source, subclip).clicked() {
                editor.make_subclip();
            }
            ui.separator();
            crate::clip_menu::items(editor, ui);
        });
        crate::theme::menu(ui, "Markers", |ui| {
            crate::sequence_menu::in_out_items(editor, ui);
            ui.separator();
            crate::marker_dialog::menu(editor, ui);
        });
        crate::theme::menu(ui, "Sequence", |ui| {
            crate::sequence_menu::sequence_menu(editor, ui)
        });
        crate::workspaces::window_menu(editor, ui);
        crate::workspaces::workspace_bar(editor, ui);
    });
}

const TOOL: egui::Vec2 = egui::vec2(26.0, 22.0);

/// One button for a group of tools, showing the one last picked from it, with an arrow
/// that lists the whole group.
fn tool_group(ui: &mut egui::Ui, current: &mut Tool, group: &'static [Tool], keymap: &Keymap) {
    let Some(&first) = group.first() else {
        return;
    };
    let id = ui.scope_id().with(("tool_group", first.label()));
    let shown = if group.contains(current) {
        ui.data_mut(|data| data.insert_temp(id, *current));
        *current
    } else {
        ui.data(|data| data.get_temp(id)).unwrap_or(first)
    };
    let button = icons::sized_toggle(ui, *current == shown, Icon::Tool(shown), Look::Filled, TOOL);
    if button.on_hover_text(tool_tip(keymap, shown)).clicked() {
        *current = shown;
    }
    if group.len() < 2 {
        return;
    }
    let (rect, arrow) = ui.allocate_exact_size(egui::vec2(10.0, 20.0), egui::Sense::click());
    let color = if arrow.hovered() {
        egui::Color32::WHITE
    } else {
        theme::WEAK_TEXT
    };
    let center = rect.center() + egui::vec2(-1.0, 1.0);
    let triangle = vec![
        center + egui::vec2(-3.0, -2.0),
        center + egui::vec2(3.0, -2.0),
        center + egui::vec2(0.0, 2.0),
    ];
    ui.painter().add(egui::Shape::convex_polygon(
        triangle,
        color,
        egui::Stroke::NONE,
    ));
    egui::Popup::menu(&arrow).show(|ui| {
        for &tool in group {
            let row = ui.horizontal(|ui| {
                let (glyph, _) =
                    ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                icons::paint(
                    ui.painter(),
                    glyph,
                    Icon::Tool(tool),
                    ui.visuals().text_color(),
                );
                ui.selectable_label(*current == tool, tool.label())
            });
            if row.inner.on_hover_text(tool_tip(keymap, tool)).clicked() {
                *current = tool;
                ui.close();
            }
        }
    });
}

fn tool_tip(keymap: &Keymap, tool: Tool) -> String {
    let text = format!("{}: {}", tool.label(), tool.hint());
    match Command::for_tool(tool) {
        Some(command) => keymap.tip(&text, command),
        None => text,
    }
}

pub fn timeline_toolbar(editor: &mut Editor, ui: &mut egui::Ui) {
    crate::sequence_tabs::tabs(editor, ui);
    crate::nesting::breadcrumbs(editor, ui);
    ui.horizontal_wrapped(|ui| {
        theme::segmented(ui, |ui| {
            for group in Tool::GROUPS {
                tool_group(ui, &mut editor.view.tool, group, &editor.keymap);
            }
        });
        theme::segmented(ui, |ui| {
            let snapping = editor.view.snapping;
            let snap = icons::sized_toggle(ui, snapping, Icon::Magnet, Look::Filled, TOOL)
                .on_hover_text(editor.keymap.tip(
                    "Snap to clip edges and the playhead",
                    Command::ToggleSnapping,
                ));
            if snap.clicked() {
                editor.view.snapping = !snapping;
            }
            let linked = editor.timeline.sequence().linked_selection == LinkedSelection::Together;
            let link = icons::sized_toggle(ui, linked, Icon::Link, Look::Filled, TOOL)
                .on_hover_text(editor.keymap.tip(
                    "Linked Selection: pick linked picture and sound together",
                    Command::ToggleLinkedSelection,
                ));
            if link.clicked() {
                editor.toggle_linked_selection();
            }
            let settings = icons::sized_toggle(ui, false, Icon::Settings, Look::Filled, TOOL)
                .on_hover_text("Timeline display settings");
            egui::Popup::menu(&settings).show(|ui| {
                ui.checkbox(
                    &mut editor.clip_art.show_thumbnails,
                    "Show Video Thumbnails",
                );
                ui.checkbox(&mut editor.clip_art.show_waveforms, "Show Audio Waveforms");
            });
        });
        theme::segmented(ui, |ui| {
            let keymap = &editor.keymap;
            let (zoom_out, fit, zoom_in) = (
                keymap.tip("Zoom out", Command::ZoomOut),
                keymap.tip("Zoom to the whole sequence", Command::ZoomToFit),
                keymap.tip("Zoom in", Command::ZoomIn),
            );
            if ui.button("−").on_hover_text(zoom_out).clicked() {
                editor.view.zoom(0.5, None);
            }
            if ui.button("Fit").on_hover_text(fit).clicked() {
                editor.view.zoom_to_fit(editor.timeline.sequence());
            }
            if ui.button("+").on_hover_text(zoom_in).clicked() {
                editor.view.zoom(2.0, None);
            }
        });
        ui.menu_button("Add Track", |ui| {
            if ui.button("Video Track").clicked() {
                editor.add_track(TrackKind::Video);
            }
            if ui.button("Audio Track").clicked() {
                editor.add_track(TrackKind::Audio);
            }
        });
    });
}
