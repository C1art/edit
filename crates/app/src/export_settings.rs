//! The export dialog's grid of format settings: preset, format, picture and sound.

use egui::ComboBox;
use media::VideoCodec;

use crate::export_format::{ExportFormat, Format, PRESETS};
use crate::export_video;

const SAMPLE_RATES: [u32; 2] = [44_100, 48_000];
const AUDIO_KBPS: [u32; 4] = [128, 192, 256, 320];

/// The export's settings as a grid, with `more` rows below them.
pub fn settings(
    ui: &mut egui::Ui,
    format: &mut ExportFormat,
    sequence_size: (u32, u32),
    more: impl FnOnce(&mut egui::Ui, &ExportFormat),
) {
    egui::Grid::new("export settings")
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            ui.label("Preset:");
            let current = format.preset_name();
            ComboBox::from_id_salt("export preset")
                .width(260.0)
                .selected_text(current.unwrap_or("Custom"))
                .show_ui(ui, |ui| {
                    for preset in &PRESETS {
                        if ui
                            .selectable_label(current == Some(preset.name), preset.name)
                            .clicked()
                        {
                            *format = ExportFormat {
                                acceleration: format.acceleration,
                                loudness: format.loudness,
                                ..preset.format
                            };
                        }
                    }
                });
            ui.end_row();
            ui.label("Format:");
            ComboBox::from_id_salt("export format")
                .width(260.0)
                .selected_text(format.format.name())
                .show_ui(ui, |ui| {
                    for kind in Format::ALL {
                        ui.selectable_value(&mut format.format, kind, kind.name());
                    }
                });
            ui.end_row();
            if format.format.has_video() {
                let codec = format.format.video_codec();
                let rate_controlled = codec.is_some_and(VideoCodec::is_rate_controlled);
                export_video::rows(ui, format, sequence_size, rate_controlled);
            }
            if format.has_audio() {
                ui.label("Sample rate:");
                match format.fixed_sample_rate() {
                    Some(rate) => {
                        ui.label(format!("{} kHz", rate as f32 / 1_000.0));
                    }
                    None => {
                        ui.horizontal(|ui| {
                            for rate in SAMPLE_RATES {
                                let text = format!("{} kHz", rate as f32 / 1_000.0);
                                ui.selectable_value(&mut format.sample_rate, rate, text);
                            }
                        });
                    }
                }
                ui.end_row();
                if format.audio_is_lossy() {
                    ui.label("Audio bitrate:");
                    ui.horizontal(|ui| {
                        for kbps in AUDIO_KBPS {
                            let text = format!("{kbps} kbps");
                            ui.selectable_value(&mut format.audio_kbps, kbps, text);
                        }
                    });
                    ui.end_row();
                }
                crate::export_loudness::row(ui, &mut format.loudness);
            }
            more(ui, format);
        });
}
