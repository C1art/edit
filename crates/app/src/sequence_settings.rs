use egui::{ComboBox, DragValue, Key};
use timeline::{SetSequenceFormat, Ticks, VideoFormat};

use crate::editor::Editor;

/// Frame sizes sequences are commonly delivered at.
const FRAME_SIZES: [(&str, u32, u32); 8] = [
    ("DCI 4K", 4096, 2160),
    ("UHD 4K", 3840, 2160),
    ("HD 1080", 1920, 1080),
    ("HD 720", 1280, 720),
    ("Vertical 1080", 1080, 1920),
    ("Square 1080", 1080, 1080),
    ("PAL SD", 720, 576),
    ("NTSC SD", 720, 480),
];

/// Within what GPUs hold in one texture.
const MAX_SIDE: u32 = 8192;

/// The sequence's frame size and rate, as Premiere's Sequence Settings dialog sets them.
pub(crate) struct SequenceSettings {
    format: VideoFormat,
}

impl Editor {
    pub(crate) fn open_sequence_settings(&mut self) {
        if self.timeline.open_nest().is_some() {
            self.notice("A nested sequence uses the project's settings");
            return;
        }
        self.sequence_settings = Some(SequenceSettings {
            format: self.timeline.sequence().format,
        });
    }
}

pub(crate) fn dialog(editor: &mut Editor, ctx: &egui::Context) {
    let Some(settings) = &mut editor.sequence_settings else {
        return;
    };
    // Clips sit on the frame grid, which a new rate would move out from under them.
    let rate_locked = editor.timeline.sequence().duration() > Ticks::ZERO;
    let mut done = false;
    let mut cancelled = false;
    crate::dialog::window(ctx, "Sequence Settings").show(ctx, |ui| {
        let format = &mut settings.format;
        egui::Grid::new("sequence settings")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label("Frame size");
                ui.horizontal(|ui| frame_size(ui, format));
                ui.end_row();
                ui.label("Timebase");
                ui.add_enabled_ui(!rate_locked, |ui| {
                    ui.horizontal(|ui| {
                        crate::interpret_dialog::rate_picker(ui, &mut format.frame_rate);
                    });
                })
                .response
                .on_disabled_hover_text("The timebase can't change once the sequence has clips");
                ui.end_row();
            });
        ui.weak("Pictures of another shape fit inside the frame, with bars.");
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
    let Some(settings) = editor.sequence_settings.take() else {
        return;
    };
    if done {
        editor.apply(SetSequenceFormat {
            format: settings.format,
        });
    }
}

fn frame_size(ui: &mut egui::Ui, format: &mut VideoFormat) {
    let shown = FRAME_SIZES
        .iter()
        .find(|&&(_, width, height)| (width, height) == (format.width, format.height))
        .map_or("Custom", |(name, ..)| name);
    ComboBox::from_id_salt("frame size")
        .selected_text(shown)
        .show_ui(ui, |ui| {
            for (name, width, height) in FRAME_SIZES {
                let chosen = (format.width, format.height) == (width, height);
                if ui.selectable_label(chosen, name).clicked() {
                    format.width = width;
                    format.height = height;
                }
            }
        });
    for side in [&mut format.width, &mut format.height] {
        let field = DragValue::new(side).range(2..=MAX_SIDE).speed(2.0);
        if ui.add(field).changed() {
            // Encoders want even sides for subsampled chroma.
            *side += *side % 2;
        }
    }
}
