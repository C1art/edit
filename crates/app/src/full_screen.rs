//! The program picture alone across the whole screen, for watching a cut back as the
//! audience will.

use egui::{CentralPanel, Color32, Frame, Rect, Sense, ViewportCommand};

use crate::editor::Editor;

impl Editor {
    pub(crate) fn toggle_full_screen(&mut self) {
        self.full_screen = !self.full_screen;
    }
}

/// Shows the program picture over everything when full screen is on, and says whether
/// it did; a double-click leaves.
pub(crate) fn show(editor: &mut Editor, ui: &mut egui::Ui) -> bool {
    let sent = ui.scope_id().with("full_screen_sent");
    if ui.data(|data| data.get_temp(sent)).unwrap_or(false) != editor.full_screen {
        ui.data_mut(|data| data.insert_temp(sent, editor.full_screen));
        ui.ctx()
            .send_viewport_cmd(ViewportCommand::Fullscreen(editor.full_screen));
    }
    if !editor.full_screen {
        return false;
    }
    let left = CentralPanel::default()
        .frame(Frame::new().fill(Color32::BLACK))
        .show(ui, |ui| {
            let area = ui.max_rect();
            let response = ui.allocate_rect(area, Sense::click());
            let format = editor.timeline.sequence().format;
            let canvas = egui::vec2(format.width as f32, format.height as f32);
            if let Some(program) = &editor.program {
                program.paint(ui.painter(), fitted(area, canvas));
            }
            response.double_clicked()
        })
        .inner;
    if left {
        editor.full_screen = false;
    }
    true
}

/// The largest rect of `canvas`'s shape centered in `area`.
fn fitted(area: Rect, canvas: egui::Vec2) -> Rect {
    let scale = (area.width() / canvas.x).min(area.height() / canvas.y);
    Rect::from_center_size(area.center(), canvas * scale)
}
