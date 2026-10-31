//! The export dialog's overlays, as Premiere's export effects: the sequence timecode
//! and an image watermark, drawn over the exported file alone.

use egui::{ComboBox, DragValue};
use timeline::{ExportOverlays, OverlayPlace, Sequence, SourceId, Watermark};

use crate::export_captions::CaptionExport;

/// The Timecode and Image Overlay rows of the export settings grid.
pub fn rows(ui: &mut egui::Ui, overlays: &mut ExportOverlays, sequence: &Sequence) {
    ui.label("Timecode overlay:");
    ui.horizontal(|ui| {
        let mut on = overlays.timecode.is_some();
        ui.checkbox(&mut on, "")
            .on_hover_text("Stamp the sequence timecode on every frame, for review copies");
        overlays.timecode = on.then(|| overlays.timecode.unwrap_or_default());
        if let Some(place) = &mut overlays.timecode {
            place_choice(ui, "timecode overlay place", place);
        }
    });
    ui.end_row();

    ui.label("Image overlay:");
    ui.horizontal(|ui| {
        let stills: Vec<(SourceId, &str)> = sequence
            .media
            .iter()
            .filter(|item| item.still && item.video.is_some())
            .map(|item| (item.id, item.name.as_str()))
            .collect();
        let chosen = overlays.watermark.map(|watermark| watermark.source);
        let name = |id| {
            stills
                .iter()
                .find(|(source, _)| *source == id)
                .map(|(_, name)| *name)
        };
        let mut picked = chosen.filter(|&id| name(id).is_some());
        ui.add_enabled_ui(!stills.is_empty(), |ui| {
            ComboBox::from_id_salt("image overlay")
                .selected_text(picked.and_then(name).unwrap_or("None"))
                .width(140.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut picked, None, "None");
                    for &(id, name) in &stills {
                        ui.selectable_value(&mut picked, Some(id), name);
                    }
                })
                .response
                .on_hover_text("A still image of the project, such as a logo")
                .on_disabled_hover_text("Import a still image, such as a logo, first");
        });
        overlays.watermark = picked.map(|source| Watermark {
            source,
            ..overlays.watermark.unwrap_or(Watermark {
                source,
                place: OverlayPlace::BottomRight,
                width: 15.0,
                opacity: 70.0,
            })
        });
        if let Some(watermark) = &mut overlays.watermark {
            place_choice(ui, "image overlay place", &mut watermark.place);
            ui.add(
                DragValue::new(&mut watermark.width)
                    .range(2.0..=60.0)
                    .speed(0.2)
                    .suffix("% wide"),
            );
            ui.add(
                DragValue::new(&mut watermark.opacity)
                    .range(0.0..=100.0)
                    .suffix("% opacity"),
            );
        }
    });
    ui.end_row();
}

fn place_choice(ui: &mut egui::Ui, id: &str, place: &mut OverlayPlace) {
    ComboBox::from_id_salt(id)
        .selected_text(place.name())
        .show_ui(ui, |ui| {
            for option in OverlayPlace::ALL {
                ui.selectable_value(place, option, option.name());
            }
        });
}

/// Whether the export's picture differs from the program's, which render previews show,
/// so that the previews can't stand in for it.
pub fn differs_from_program(
    overlays: &ExportOverlays,
    captions: CaptionExport,
    sequence: &Sequence,
) -> bool {
    let captions_differ = !sequence.captions.items.is_empty()
        && (captions == CaptionExport::BurnIn) == sequence.captions.hidden;
    !overlays.is_empty() || captions_differ
}
