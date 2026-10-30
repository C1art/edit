use egui::{Color32, Rect, Sense, UiBuilder};
use playback::Resolution;
use timeline::PlaceMode;

use crate::editor::Editor;
use crate::icons::Icon;
use crate::keymap::{Command, Keymap};
use crate::monitor_drop;
use crate::placing;
use crate::program_zoom::MonitorZoom;
use crate::project_files::PathPurpose;
use crate::source_drag;
use crate::source_waveform;
use crate::theme;
use crate::transport::{self, Buttons, Extra};
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
    let focus = editor.focus;
    // Columns narrower than the gap between them have no width at all.
    if ui.available_width() <= 2.0 * ui.spacing().item_spacing.x {
        return;
    }
    ui.columns(2, |columns| {
        // Each monitor measures its own controls; the columns alone don't tell them apart.
        theme::focus_card(&mut columns[0], focus == Focus::Source, |ui| {
            ui.push_id("source", |ui| {
                fitted(ui, |ui| source(editor, ui));
                monitor_drop::source(editor, ui, ui.min_rect());
            });
        });
        theme::focus_card(&mut columns[1], focus == Focus::Program, |ui| {
            ui.push_id("program", |ui| {
                fitted(ui, |ui| program(editor, ui));
                let mut picture = ui.min_rect();
                if let Some(bottom) = ui.data(|data| data.get_temp(picture_bottom_id(ui))) {
                    picture.max.y = bottom;
                }
                monitor_drop::program(editor, ui, picture);
            });
        });
    });
}

fn controls_id(ui: &egui::Ui) -> egui::Id {
    ui.scope_id().with("controls_height")
}

fn picture_bottom_id(ui: &egui::Ui) -> egui::Id {
    ui.scope_id().with("picture_bottom")
}

/// Shows a monitor, then measures what it showed under its picture, so that its
/// picture leaves room for buttons wrapped onto more rows in a narrow monitor.
fn fitted(ui: &mut egui::Ui, monitor: impl FnOnce(&mut egui::Ui)) {
    let bottom_id = picture_bottom_id(ui);
    ui.data_mut(|data| data.remove::<f32>(bottom_id));
    monitor(ui);
    let Some(picture_bottom) = ui.data(|data| data.get_temp::<f32>(bottom_id)) else {
        return;
    };
    let id = controls_id(ui);
    // The card holding the monitor fills its panel, so what the monitor used is up to
    // the cursor, not the card's whole rect.
    let used = ui.cursor().top() - ui.spacing().item_spacing.y;
    let measured = (used - picture_bottom).max(0.0);
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

/// The room for the picture, which must be shown at the cursor.
fn picture_size(ui: &egui::Ui) -> egui::Vec2 {
    let controls = ui
        .data(|data| data.get_temp(controls_id(ui)))
        .unwrap_or(CONTROLS_HEIGHT);
    let size = egui::vec2(
        ui.available_width(),
        (ui.available_height() - controls).max(SMALLEST_PICTURE),
    );
    let bottom = ui.cursor().top() + size.y;
    ui.data_mut(|data| data.insert_temp(picture_bottom_id(ui), bottom));
    size
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
            "Click or drag an item from the project to open it here",
            egui::FontId::proportional(13.0),
            ui.visuals().weak_text_color(),
        );
        return;
    };
    let item = editor.timeline.sequence().media_item(source.source);
    let has_both = item.is_some_and(|item| item.video.is_some() && item.has_audio);
    let sound_only = item.filter(|item| item.video.is_none() && item.has_audio);
    ui.horizontal(|ui| {
        title(ui, &format!("Source: {}", source.name), focused);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            source_drag::handles(ui, source, has_both);
        });
    });
    let picture = source.viewer.picture(ui, picture_size(ui));
    if let Some(item) = sound_only {
        source_waveform::paint(
            ui.painter(),
            picture.rect,
            source,
            item,
            &mut editor.clip_art,
        );
    }
    let picture = source_drag::picture(ui, source, &picture);
    if sound_only.is_some() {
        source_waveform::seek_on_click(&picture, source);
    }
    let marks = source.marks;
    let keymap = &editor.keymap;
    let before = mark_buttons(keymap);
    let after = [
        extra(
            keymap,
            Icon::Insert,
            "Insert",
            Command::Insert,
            Action::Insert,
        ),
        extra(
            keymap,
            Icon::Overwrite,
            "Overwrite",
            Command::Overwrite,
            Action::Overwrite,
        ),
        extra(
            keymap,
            Icon::Subclip,
            "Make a subclip of the marked part",
            Command::MakeSubclip,
            Action::Subclip,
        ),
    ];
    let buttons = Buttons {
        before: &before,
        after: &after,
        settings: false,
    };
    let pressed = transport::ui(ui, &mut source.viewer.player, Some(marks), keymap, buttons);
    let mut place = None;
    let mut subclip = false;
    match pressed.action {
        Some(Action::MarkIn) => source.mark_in(),
        Some(Action::MarkOut) => source.mark_out(),
        Some(Action::ClearMarks) => source.marks = timeline::Marks::default(),
        Some(Action::Insert) => place = Some(PlaceMode::Insert),
        Some(Action::Overwrite) => place = Some(PlaceMode::Overwrite),
        Some(Action::Subclip) => subclip = true,
        _ => {}
    }
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
    let name = format!("Program: {}", editor.shown_sequence_name());
    title(ui, &name, focused);
    let Some(program) = &mut editor.program else {
        return;
    };
    program.set_transparency_grid(editor.transparency_grid);
    if trim_picture(editor, ui).is_none()
        && crate::comparison::ui(editor, ui, picture_size(ui)).is_none()
    {
        zoomed_program(editor, ui);
    }
    let Some(program) = &mut editor.program else {
        return;
    };
    let marks = editor.timeline.sequence().marks;
    let keymap = &editor.keymap;
    let before = mark_buttons(keymap);
    let after = [
        extra(
            keymap,
            Icon::Lift,
            "Lift",
            Command::LiftMarked,
            Action::Lift,
        ),
        extra(
            keymap,
            Icon::Extract,
            "Extract",
            Command::ExtractMarked,
            Action::Extract,
        ),
        extra(
            keymap,
            Icon::Camera,
            "Export Frame: save the picture at the playhead as an image",
            Command::ExportFrame,
            Action::ExportFrame,
        ),
    ];
    let buttons = Buttons {
        before: &before,
        after: &after,
        settings: true,
    };
    let pressed = transport::ui(ui, &mut program.player, Some(marks), keymap, buttons);
    match pressed.action {
        Some(Action::MarkIn) => editor.mark_sequence_in(),
        Some(Action::MarkOut) => editor.mark_sequence_out(),
        Some(Action::ClearMarks) => editor.clear_sequence_marks(),
        Some(Action::Lift) => editor.lift_marked(),
        Some(Action::Extract) => editor.extract_marked(),
        Some(Action::ExportFrame) => editor.prompt_path(PathPurpose::ExportFrame),
        _ => {}
    }
    if let Some(settings) = pressed.settings {
        egui::Popup::menu(&settings).show(|ui| {
            ui.checkbox(&mut editor.safe_margins, "Safe Margins");
            ui.checkbox(&mut editor.transparency_grid, "Transparency Grid")
                .on_hover_text("A checkerboard where nothing covers the frame");
            let mut ganged = editor.ganged;
            let toggle = ui
                .checkbox(&mut ganged, "Gang Source and Program")
                .on_hover_text("Moving either playhead moves the other by as much");
            if toggle.clicked() {
                editor.toggle_gang();
            }
            ui.checkbox(
                &mut editor.dropped_frame_indicator,
                "Show Dropped Frame Indicator",
            );
            let mut multicam = editor.multicam_view;
            let toggle = ui
                .checkbox(&mut multicam, "Multi-Camera")
                .on_hover_text(editor.keymap.tip(MULTICAM_TIP, Command::MulticamView));
            if toggle.clicked() {
                editor.toggle_multicam_view();
            }
            let mut comparing = editor.comparison.is_some();
            let toggle = ui
                .checkbox(&mut comparing, "Comparison View")
                .on_hover_text(COMPARISON_TIP);
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
    }
    if pressed_inside(ui) {
        editor.focus = Focus::Program;
    }
}

const MULTICAM_TIP: &str = "Every camera of the multi-camera clips beside the program; click \
                            one or press 1–9 to switch";
const COMPARISON_TIP: &str =
    "A reference frame of the sequence beside the program, for matching shots";

#[derive(Debug, Clone, Copy)]
enum Action {
    MarkIn,
    MarkOut,
    ClearMarks,
    Insert,
    Overwrite,
    Subclip,
    Lift,
    Extract,
    ExportFrame,
}

fn extra(
    keymap: &Keymap,
    icon: Icon,
    tip: &str,
    command: Command,
    action: Action,
) -> Extra<Action> {
    Extra {
        icon,
        tip: keymap.tip(tip, command),
        action,
    }
}

fn mark_buttons(keymap: &Keymap) -> [Extra<Action>; 3] {
    [
        extra(
            keymap,
            Icon::MarkIn,
            "Mark In",
            Command::MarkIn,
            Action::MarkIn,
        ),
        extra(
            keymap,
            Icon::MarkOut,
            "Mark Out",
            Command::MarkOut,
            Action::MarkOut,
        ),
        extra(
            keymap,
            Icon::ClearMarks,
            "Clear In and Out",
            Command::ClearInOut,
            Action::ClearMarks,
        ),
    ]
}

/// The program picture at the chosen zoom, clipped to the monitor.
fn zoomed_program(editor: &mut Editor, ui: &mut egui::Ui) {
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
    // Handles past the monitor's edge neither show nor take the pointer.
    let mut clipped = ui.new_child(UiBuilder::new().id_salt("program_overlays").max_rect(area));
    clipped.set_clip_rect(area.intersect(ui.clip_rect()));
    program_overlays(editor, &mut clipped, &response, picture);
    if let Some(program) = editor
        .program
        .as_ref()
        .filter(|_| editor.dropped_frame_indicator)
    {
        crate::dropped_frames::indicator(&mut clipped, area, &program.player);
    }
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
            crate::effects::matte_overlay(editor, ui, picture);
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
