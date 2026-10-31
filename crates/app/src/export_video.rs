//! The export dialog's rows for a format with pictures.

use egui::{ComboBox, DragValue, Slider};
use media::{Acceleration, RateControl};

use crate::export_format::ExportFormat;

/// Frame heights offered besides the sequence's own.
const HEIGHTS: [u32; 6] = [2160, 1440, 1080, 720, 540, 360];

pub fn rows(
    ui: &mut egui::Ui,
    format: &mut ExportFormat,
    sequence_size: (u32, u32),
    rate_controlled: bool,
) {
    ui.label("Frame size:");
    let size_text = |height: Option<u32>| {
        let (width, height) = ExportFormat { height, ..*format }.size(sequence_size);
        if (width, height) == sequence_size {
            format!("{width}×{height} (sequence)")
        } else {
            format!("{width}×{height}")
        }
    };
    ComboBox::from_id_salt("export size")
        .width(260.0)
        .selected_text(size_text(format.height))
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut format.height, None, size_text(None));
            for height in HEIGHTS
                .into_iter()
                .filter(|&height| height != sequence_size.1)
            {
                ui.selectable_value(&mut format.height, Some(height), size_text(Some(height)));
            }
        });
    ui.end_row();
    let (width, height) = sequence_size;
    if format.preset().is_some_and(|preset| preset.vertical) && width > height {
        ui.label("");
        ui.colored_label(
            ui.visuals().warn_fg_color,
            "This sequence is landscape. Sequence > Auto Reframe Sequence makes a vertical copy.",
        );
        ui.end_row();
    }
    if rate_controlled {
        ui.label("Bitrate:");
        ui.horizontal(|ui| {
            let by_quality = matches!(format.rate_control, RateControl::Quality(_));
            if ui.radio(by_quality, "Quality").clicked() && !by_quality {
                format.rate_control = RateControl::Quality(18);
            }
            if ui.radio(!by_quality, "Target").clicked() && by_quality {
                format.rate_control = RateControl::Bitrate(16_000_000);
            }
            match &mut format.rate_control {
                RateControl::Quality(crf) => {
                    ui.add(Slider::new(crf, 0..=40).text("CRF, lower is better"));
                }
                RateControl::Bitrate(bits) => {
                    let mut mbps = *bits as f32 / 1e6;
                    let drag = DragValue::new(&mut mbps)
                        .range(1.0..=400.0)
                        .speed(0.2)
                        .max_decimals(1)
                        .suffix(" Mbps");
                    if ui.add(drag).changed() {
                        *bits = (mbps * 1e6).round() as u32;
                    }
                }
            }
        });
        ui.end_row();
        ui.label("Encoding:");
        ui.horizontal(|ui| {
            let choices = [
                (
                    Acceleration::Hardware,
                    "Hardware",
                    "The GPU's encoder when there is one; software otherwise",
                ),
                (
                    Acceleration::Software,
                    "Software",
                    "Slower, and the same on every machine",
                ),
            ];
            for (acceleration, name, hint) in choices {
                ui.radio_value(&mut format.acceleration, acceleration, name)
                    .on_hover_text(hint);
            }
        });
        ui.end_row();
    }
    if format.format.audio_codec().is_some() {
        ui.label("Audio:");
        ui.checkbox(&mut format.with_audio, "Export audio");
        ui.end_row();
    }
}
