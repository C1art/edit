use egui::Key;
use timeline::{FrameRate, SetMediaTimecode, SourceId, TimeDisplay, Timebase};

use crate::editor::Editor;

/// Sets the timecode a file's first frame shows, as Premiere's Modify > Timecode does
/// for a camera whose clock was wrong or a file that carries none.
pub(crate) struct TimecodeDialog {
    source: SourceId,
    name: String,
    /// Frames at the item's own rate.
    start: i64,
    format: (Timebase, FrameRate, TimeDisplay),
}

impl Editor {
    pub(crate) fn open_media_timecode(&mut self, source: SourceId) {
        let project = self.timeline.project();
        let Some(item) = project.media_item(source) else {
            return;
        };
        let Some(format) = item.played_format().filter(|_| item.is_file()) else {
            self.notice("Only video files carry timecode");
            return;
        };
        self.timecode_dialog = Some(TimecodeDialog {
            source,
            name: item.name.clone(),
            start: item.timecode.unwrap_or(0),
            format: (project.timebase, format.frame_rate, project.time_display),
        });
    }
}

pub(crate) fn dialog(editor: &mut Editor, ctx: &egui::Context) {
    let Some(dialog) = &mut editor.timecode_dialog else {
        return;
    };
    let mut done = false;
    let mut cancelled = false;
    crate::dialog::window(ctx, "Modify Timecode").show(ctx, |ui| {
        ui.strong(&dialog.name);
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("First frame");
            let field = crate::timecode_field::frames(&mut dialog.start, dialog.format);
            ui.add(field.range(0..=i64::MAX))
                .on_hover_text("The timecode the file's first frame shows");
            if ui.button("Reset").clicked() {
                dialog.start = 0;
            }
        });
        ui.weak("Monitors, the metadata panel and EDLs read media time from here.");
        crate::dialog::footer(ui, |ui| {
            cancelled =
                ui.button("Cancel").clicked() || ui.input(|input| input.key_pressed(Key::Escape));
            done = ui.add(crate::dialog::primary("OK")).clicked()
                || ui.input(|input| input.key_pressed(Key::Enter));
        });
    });
    if !(done || cancelled) {
        return;
    }
    let Some(dialog) = editor.timecode_dialog.take() else {
        return;
    };
    if done {
        editor.apply_to_pool(SetMediaTimecode {
            source: dialog.source,
            timecode: Some(dialog.start),
        });
    }
}
