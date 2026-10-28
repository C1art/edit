use egui::{Color32, Stroke, StrokeKind};
use timeline::PlaceMode;

use crate::editor::Editor;
use crate::placing;
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
    });
    if pressed_inside(ui) {
        editor.focus = Focus::Source;
    }
    if let Some(mode) = place {
        placing::edit_from_source(editor, mode);
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
    });
    if pressed_inside(ui) {
        editor.focus = Focus::Program;
    }
}

fn pressed_inside(ui: &egui::Ui) -> bool {
    ui.rect_contains_pointer(ui.min_rect()) && ui.input(|input| input.pointer.any_pressed())
}
