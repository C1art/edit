//! The program picture across the whole screen with a video player's controls, for
//! watching a cut back as the audience will.

use egui::{CentralPanel, Color32, Frame, Key, Modifiers, Rect, Sense, ViewportCommand};
use playback::PlayRate;

use crate::editor::Editor;
use crate::full_screen_controls::{self, SKIP_SECONDS};
use crate::monitors::Focus;

impl Editor {
    pub(crate) fn toggle_full_screen(&mut self) {
        if self.full_screen {
            self.leave_full_screen();
        } else {
            self.full_screen = true;
            // Space and the transport keys then work the picture on screen.
            self.focus = Focus::Program;
        }
    }

    /// Back to the editor, where playing at another speed would only confuse editing.
    fn leave_full_screen(&mut self) {
        self.full_screen = false;
        if let Some(program) = &mut self.program {
            program.player.set_speed(PlayRate::NORMAL);
        }
    }
}

/// Keys a video player answers to, ahead of the editor's own: Escape leaves and the
/// left and right arrows skip back and forward.
pub(crate) fn keys(editor: &mut Editor, ctx: &egui::Context) {
    let (leave, back, forward) = ctx.input_mut(|input| {
        (
            input.consume_key(Modifiers::NONE, Key::Escape),
            input.consume_key(Modifiers::NONE, Key::ArrowLeft),
            input.consume_key(Modifiers::NONE, Key::ArrowRight),
        )
    });
    if leave {
        editor.leave_full_screen();
        return;
    }
    if let Some(program) = &mut editor.program {
        for (pressed, seconds) in [(back, -SKIP_SECONDS), (forward, SKIP_SECONDS)] {
            if pressed {
                full_screen_controls::skip(&mut program.player, seconds);
            }
        }
    }
}

/// Shows the program picture and its controls over everything when full screen is on,
/// and says whether it did. Clicking the picture plays or pauses; a double-click leaves.
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
            let Some(program) = &mut editor.program else {
                return response.double_clicked();
            };
            program.paint(ui.painter(), fitted(area, canvas));
            // Both clicks of a double-click toggle, so leaving keeps playing as it was.
            if response.clicked() {
                program.player.toggle();
            }
            let asked = full_screen_controls::show(ui, area, &mut program.player);
            asked || response.double_clicked()
        })
        .inner;
    if left {
        editor.leave_full_screen();
    }
    true
}

/// The largest rect of `canvas`'s shape centered in `area`.
fn fitted(area: Rect, canvas: egui::Vec2) -> Rect {
    let scale = (area.width() / canvas.x).min(area.height() / canvas.y);
    Rect::from_center_size(area.center(), canvas * scale)
}
