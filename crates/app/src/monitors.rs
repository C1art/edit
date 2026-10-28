use egui::{Color32, Stroke, StrokeKind};
use playback::Resolution;
use timeline::PlaceMode;

use crate::editor::Editor;
use crate::placing;
use crate::project_files::PathPurpose;
use crate::transport;

/// Space below each picture for its scrub bar, transport and buttons.
const CONTROLS_HEIGHT: f32 = 84.0;

/// Which monitor the transport keys drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Source,
    Program,
}

/// The source monitor on the left and the program monitor on the right.
pub fn ui(editor: &mut Editor, ui: &mut egui::Ui) {
    ui.columns(2, |columns| {
        source(editor, &mut columns[0]);
        program(editor, &mut columns[1]);
    });
}

fn title(ui: &mut egui::Ui, text: &str, focused: bool) {
    let color = if focused {
        ui.visuals().strong_text_color()
    } else {
        ui.visuals().weak_text_color()
    };
    ui.colored_label(color, text);
}

fn picture_size(ui: &egui::Ui) -> egui::Vec2 {
    egui::vec2(
        ui.available_width(),
        (ui.available_height() - CONTROLS_HEIGHT).max(60.0),
    )
}

fn outline(ui: &egui::Ui, response: &egui::Response, focused: bool) {
    if focused {
        ui.painter().rect_stroke(
            response.rect,
            0.0,
            Stroke::new(1.5, Color32::from_rgb(80, 160, 255)),
            StrokeKind::Outside,
        );
    }
}

fn source(editor: &mut Editor, ui: &mut egui::Ui) {
    let focused = editor.focus == Focus::Source;
    let Some(source) = &mut editor.source else {
        title(ui, "Source", focused);
        let (rect, _) = ui.allocate_exact_size(picture_size(ui), egui::Sense::hover());
        ui.painter().rect_filled(rect, 0.0, Color32::BLACK);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Click an item in the project to open it here",
            egui::FontId::proportional(13.0),
            ui.visuals().weak_text_color(),
        );
        return;
    };
    title(ui, &format!("Source: {}", source.name), focused);
    let response = source.viewer.picture(ui, picture_size(ui));
    outline(ui, &response, focused);
    let marks = source.marks;
    transport::ui(ui, &mut source.viewer.player, Some(marks));
    let mut place = None;
    let mut subclip = false;
    ui.horizontal(|ui| {
        if ui.button("Mark In").on_hover_text("I").clicked() {
            source.mark_in();
        }
        if ui.button("Mark Out").on_hover_text("O").clicked() {
            source.mark_out();
        }
        if ui
            .button("Clear")
            .on_hover_text("Clear in and out")
            .clicked()
        {
            source.marks = timeline::Marks::default();
        }
        ui.separator();
        if ui.button("Insert").on_hover_text(",").clicked() {
            place = Some(PlaceMode::Insert);
        }
        if ui.button("Overwrite").on_hover_text(".").clicked() {
            place = Some(PlaceMode::Overwrite);
        }
        ui.separator();
        subclip = ui
            .button("Subclip")
            .on_hover_text("Make a subclip of the marked part (Ctrl+U)")
            .clicked();
    });
    if pressed_inside(ui) {
        editor.focus = Focus::Source;
    }
    if let Some(mode) = place {
        placing::edit_from_source(editor, mode);
    }
    if subclip {
        editor.make_subclip();
    }
}

fn program(editor: &mut Editor, ui: &mut egui::Ui) {
    let focused = editor.focus == Focus::Program;
    title(ui, "Program", focused);
    let Some(program) = &mut editor.program else {
        return;
    };
    let response = program.picture(ui, picture_size(ui));
    outline(ui, &response, focused);
    if editor.multicam_layout.is_some() {
        let clicked = response
            .clicked()
            .then(|| response.interact_pointer_pos())
            .flatten();
        crate::multicam::overlay(editor, ui, response.rect, clicked);
    } else {
        if editor.safe_margins {
            let format = editor.timeline.sequence().format;
            let canvas = egui::vec2(format.width as f32, format.height as f32);
            crate::safe_margins::paint(ui, response.rect, canvas);
        }
        crate::effects::motion_overlay(editor, ui, response.rect);
        crate::effects::mask_overlay(editor, ui, response.rect);
    }
    let Some(program) = &mut editor.program else {
        return;
    };
    let marks = editor.timeline.sequence().marks;
    transport::ui(ui, &mut program.player, Some(marks));
    ui.horizontal(|ui| {
        if ui.button("Mark In").on_hover_text("I").clicked() {
            editor.mark_sequence_in();
        }
        if ui.button("Mark Out").on_hover_text("O").clicked() {
            editor.mark_sequence_out();
        }
        if ui
            .button("Clear")
            .on_hover_text("Clear in and out (Ctrl+Shift+X)")
            .clicked()
        {
            editor.clear_sequence_marks();
        }
        ui.separator();
        if ui.button("Lift").on_hover_text(";").clicked() {
            editor.lift_marked();
        }
        if ui.button("Extract").on_hover_text("'").clicked() {
            editor.extract_marked();
        }
        ui.separator();
        let export_frame = ui
            .button("Still")
            .on_hover_text("Export Frame: save the picture at the playhead as an image (Ctrl+Shift+E)");
        if export_frame.clicked() {
            editor.prompt_path(PathPurpose::ExportFrame);
        }
        ui.menu_button("View", |ui| {
            ui.checkbox(&mut editor.safe_margins, "Safe Margins");
            let mut multicam = editor.multicam_view;
            let toggle = ui
                .checkbox(&mut multicam, "Multi-Camera")
                .on_hover_text("Every camera of the multi-camera clips beside the program; click one or press 1–9 to switch (Shift+0)");
            if toggle.clicked() {
                editor.toggle_multicam_view();
            }
            ui.menu_button("Playback Resolution", |ui| {
                let Some(program) = &mut editor.program else {
                    return;
                };
                let current = program.player.resolution();
                for resolution in Resolution::ALL {
                    if ui.radio(current == resolution, resolution.name()).clicked() {
                        program.player.set_resolution(resolution);
                    }
                }
            });
        });
    });
    if pressed_inside(ui) {
        editor.focus = Focus::Program;
    }
}

fn pressed_inside(ui: &egui::Ui) -> bool {
    ui.rect_contains_pointer(ui.min_rect()) && ui.input(|input| input.pointer.any_pressed())
}
