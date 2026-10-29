use egui::{Color32, Rect, Sense, Stroke, StrokeKind, UiBuilder};
use playback::Resolution;
use timeline::PlaceMode;

use crate::editor::Editor;
use crate::placing;
use crate::program_zoom::MonitorZoom;
use crate::project_files::PathPurpose;
use crate::transport;
use crate::trim_monitor::TrimMonitor;

/// Space below each picture for its scrub bar, transport and buttons, until they
/// have been measured.
const CONTROLS_HEIGHT: f32 = 84.0;
const SMALLEST_PICTURE: f32 = 60.0;

/// Which monitor the transport keys drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Source,
    Program,
}

/// The source monitor on the left and the program monitor on the right.
pub fn ui(editor: &mut Editor, ui: &mut egui::Ui) {
    ui.columns(2, |columns| {
        fitted(&mut columns[0], |ui| source(editor, ui));
        fitted(&mut columns[1], |ui| program(editor, ui));
    });
}

fn controls_id(ui: &egui::Ui) -> egui::Id {
    ui.scope_id().with("controls_height")
}

/// Shows a monitor, then measures what it showed besides its picture, so that its
/// picture leaves room for buttons wrapped onto more rows in a narrow monitor.
fn fitted(ui: &mut egui::Ui, monitor: impl FnOnce(&mut egui::Ui)) {
    let top = ui.cursor().top();
    let picture = picture_size(ui).y;
    monitor(ui);
    let id = controls_id(ui);
    let measured = (ui.min_rect().bottom() - top - picture).max(0.0);
    let controls = ui.data(|data| data.get_temp(id)).unwrap_or(CONTROLS_HEIGHT);
    if (measured - controls).abs() > 0.5 {
        ui.data_mut(|data| data.insert_temp(id, measured));
        ui.ctx().request_repaint();
    }
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
    let controls = ui
        .data(|data| data.get_temp(controls_id(ui)))
        .unwrap_or(CONTROLS_HEIGHT);
    egui::vec2(
        ui.available_width(),
        (ui.available_height() - controls).max(SMALLEST_PICTURE),
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
    ui.horizontal_wrapped(|ui| {
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
    if editor.program.is_none() {
        return;
    }
    if let Some(response) = trim_picture(editor, ui) {
        outline(ui, &response, focused);
    } else if let Some(response) = crate::comparison::ui(editor, ui, picture_size(ui)) {
        outline(ui, &response, focused);
    } else {
        zoomed_program(editor, ui, focused);
    }
    let Some(program) = &mut editor.program else {
        return;
    };
    let marks = editor.timeline.sequence().marks;
    transport::ui(ui, &mut program.player, Some(marks));
    ui.horizontal_wrapped(|ui| {
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
            let mut comparing = editor.comparison.is_some();
            let toggle = ui
                .checkbox(&mut comparing, "Comparison View")
                .on_hover_text("A reference frame of the sequence beside the program, for matching shots");
            if toggle.clicked() {
                editor.toggle_comparison_view();
            }
            ui.menu_button("Zoom", |ui| {
                let current = editor.program_zoom.zoom();
                for zoom in MonitorZoom::ALL {
                    if ui.radio(current == zoom, zoom.name()).clicked() {
                        editor.program_zoom.set_zoom(zoom);
                    }
                }
            });
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

/// The program picture at the chosen zoom, clipped to the monitor.
fn zoomed_program(editor: &mut Editor, ui: &mut egui::Ui, focused: bool) {
    let Some(program) = &editor.program else {
        return;
    };
    let (area, response) = ui.allocate_exact_size(picture_size(ui), Sense::click_and_drag());
    let format = editor.timeline.sequence().format;
    let canvas = egui::vec2(format.width as f32, format.height as f32);
    // The camera grid is laid out over the whole monitor.
    let picture = if editor.multicam_layout.is_some() {
        area
    } else {
        editor.program_zoom.place(ui, &response, canvas)
    };
    let painter = ui.painter_at(area);
    painter.rect_filled(area, 0.0, Color32::BLACK);
    program.paint(&painter, picture);
    outline(ui, &response, focused);
    // Handles past the monitor's edge neither show nor take the pointer.
    let mut clipped = ui.new_child(UiBuilder::new().id_salt("program_overlays").max_rect(area));
    clipped.set_clip_rect(area.intersect(ui.clip_rect()));
    program_overlays(editor, &mut clipped, &response, picture);
}

/// What the program picture shows on top: the camera grid, or safe margins and the
/// handles of the selected clip's motion and masks.
fn program_overlays(
    editor: &mut Editor,
    ui: &mut egui::Ui,
    response: &egui::Response,
    picture: Rect,
) {
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
            crate::safe_margins::paint(ui, picture, canvas);
        }
        if !crate::graphics_tools::overlay(editor, ui, picture) {
            crate::effects::motion_overlay(editor, ui, picture);
            crate::effects::mask_overlay(editor, ui, picture);
        }
    }
}

/// The trim monitor in place of the program picture while an edit is dragged.
fn trim_picture(editor: &mut Editor, ui: &mut egui::Ui) -> Option<egui::Response> {
    let Some(view) = editor.view.trim_view() else {
        if let Some(trim) = &mut editor.trim_monitor {
            trim.reset();
        }
        return None;
    };
    if editor.trim_monitor.is_none() {
        editor.trim_monitor = editor.render_state.as_ref().and_then(TrimMonitor::new);
    }
    let trim = editor.trim_monitor.as_mut()?;
    let (rect, response) = ui.allocate_exact_size(picture_size(ui), egui::Sense::click());
    trim.ui(ui, rect, &view);
    Some(response)
}

fn pressed_inside(ui: &egui::Ui) -> bool {
    ui.rect_contains_pointer(ui.min_rect()) && ui.input(|input| input.pointer.any_pressed())
}
