use std::ops::{Range, RangeInclusive};

use egui::Key;
use timeline::{EditSubclip, FrameRate, MediaItem, SourceId, Speed, Ticks, TimeDisplay, Timebase};

use crate::editor::Editor;

/// Edit Subclip: moves where a subclip starts and ends in its file, or makes it stand
/// for the whole file again.
pub(crate) struct SubclipDialog {
    source: SourceId,
    name: String,
    frames: Frames,
    /// The subclip's first and last frames, as the file's timecode counts them.
    start: i64,
    end: i64,
    to_master: bool,
}

/// How the file's time reads as frames of its timecode.
#[derive(Clone, Copy)]
struct Frames {
    timebase: Timebase,
    rate: FrameRate,
    display: TimeDisplay,
    first: Ticks,
    conform: Speed,
    duration: Ticks,
}

impl Frames {
    fn of(item: &MediaItem, (timebase, rate, display): (Timebase, FrameRate, TimeDisplay)) -> Self {
        Self {
            timebase,
            rate: item
                .played_format()
                .map_or(rate, |format| format.frame_rate),
            display,
            first: item.start_timecode(timebase),
            conform: item.conform(),
            duration: item.duration,
        }
    }

    /// The frame showing at `time` in the file.
    fn frame(&self, time: Ticks) -> i64 {
        let shown = self.first + self.conform.sequence_span(time);
        self.rate.frame_at(shown, self.timebase)
    }

    /// Where `frame` starts in the file.
    fn time(&self, frame: i64) -> Ticks {
        let shown = self.rate.frame_start(frame, self.timebase) - self.first;
        self.conform
            .source_span(shown)
            .clamp(Ticks::ZERO, self.duration)
    }

    fn whole(&self) -> RangeInclusive<i64> {
        self.frame(Ticks::ZERO)..=self.frame(self.duration - Ticks(1))
    }

    /// The file time from the start of frame `start` to the end of frame `end`.
    fn span(&self, start: i64, end: i64) -> Range<Ticks> {
        self.time(start)..self.time(end + 1)
    }

    fn format(&self) -> (Timebase, FrameRate, TimeDisplay) {
        (self.timebase, self.rate, self.display)
    }
}

impl Editor {
    pub(crate) fn open_edit_subclip(&mut self, source: SourceId) {
        let project = self.timeline.project();
        let Some(item) = project.media_item(source) else {
            return;
        };
        let Some(range) = item.subclip.clone() else {
            self.notice("Only subclips have a range to edit");
            return;
        };
        let format = (
            project.timebase,
            project.format.frame_rate,
            project.time_display,
        );
        let frames = Frames::of(item, format);
        self.subclip_dialog = Some(SubclipDialog {
            source,
            name: item.name.clone(),
            frames,
            start: frames.frame(range.start),
            end: frames.frame(range.end - Ticks(1)),
            to_master: false,
        });
    }
}

pub(crate) fn dialog(editor: &mut Editor, ctx: &egui::Context) {
    let Some(dialog) = &mut editor.subclip_dialog else {
        return;
    };
    let mut done = false;
    let mut cancelled = false;
    crate::dialog::window(ctx, "Edit Subclip").show(ctx, |ui| {
        ui.strong(&dialog.name);
        ui.separator();
        let frames = dialog.frames;
        let whole = frames.whole();
        let timecode = |frame| crate::timecode_field::frames_text(frame, frames.format());
        ui.weak(format!(
            "Master clip {} – {}",
            timecode(*whole.start()),
            timecode(*whole.end())
        ));
        let (start, end) = (&mut dialog.start, &mut dialog.end);
        ui.add_enabled_ui(!dialog.to_master, |ui| {
            egui::Grid::new("edit subclip")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    ui.label("Start");
                    let field = crate::timecode_field::frames(start, frames.format());
                    ui.add(field.range(*whole.start()..=*end));
                    ui.end_row();
                    ui.label("End");
                    let field = crate::timecode_field::frames(end, frames.format());
                    ui.add(field.range(*start..=*whole.end()));
                    ui.end_row();
                });
        });
        ui.checkbox(&mut dialog.to_master, "Convert to Master Clip")
            .on_hover_text("The item stands for the whole file again");
        ui.weak("Clips already in sequences keep the frames they show.");
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
    let Some(dialog) = editor.subclip_dialog.take() else {
        return;
    };
    if done {
        let range = (!dialog.to_master).then(|| dialog.frames.span(dialog.start, dialog.end));
        editor.apply_to_pool(EditSubclip {
            source: dialog.source,
            range,
        });
    }
}
