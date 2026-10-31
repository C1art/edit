//! Quick Export: the sequence straight to a file from the menu bar, with a name and a
//! few everyday presets, written in the background through the export queue so
//! editing carries on.

use std::path::Path;

use egui::{Key, PopupCloseBehavior, RichText};

use crate::editor::Editor;
use crate::export::{self, ExportDialog};
use crate::export_captions::sidecar_path;
use crate::export_format::{ExportFormat, PRESETS};
use crate::export_summary::summary;
use crate::export_target::{Holder, Target};
use crate::project_files::quoted_name;

/// Offered by name, so the full list stays in one place; wide and tall sequences get
/// the ones made for their shape.
const WIDE: [&str; 4] = [
    "Match Source - High Bitrate",
    "YouTube 1080p Full HD",
    "Vimeo 720p HD",
    "MP3 320 kbps",
];
const TALL: [&str; 5] = [
    "Match Source - High Bitrate",
    "YouTube Shorts 1080×1920",
    "TikTok 1080×1920",
    "Instagram Reels 1080×1920",
    "MP3 320 kbps",
];

pub(crate) struct QuickExport {
    export: ExportDialog,
}

/// The Quick Export button, showing how far a background export has got.
pub(crate) fn button(editor: &mut Editor, ui: &mut egui::Ui) {
    let progress = editor.export_queue.progress();
    let text = match progress {
        Some((fraction, 0)) => format!("Exporting {:.0}%", fraction * 100.0),
        Some((fraction, waiting)) => {
            format!("Exporting {:.0}% (+{waiting})", fraction * 100.0)
        }
        None => "Quick Export".to_owned(),
    };
    ui.add_space(6.0);
    // Outlined, so it doesn't read as one more workspace tab, and wide enough for its
    // progress that the tabs beside it hold still.
    let outline = ui
        .visuals()
        .widgets
        .inactive
        .fg_stroke
        .color
        .gamma_multiply(0.4);
    let button = egui::Button::new(text)
        .stroke(egui::Stroke::new(1.0, outline))
        .min_size(egui::vec2(124.0, 0.0));
    let response = ui.add(button).on_hover_text(if progress.is_some() {
        "Export the sequence again, or see the export queue"
    } else {
        "Export the sequence to a video file"
    });
    if response.clicked() {
        editor.quick_export = match editor.quick_export.take() {
            Some(_) => None,
            None => Some(QuickExport {
                export: ExportDialog::new(editor),
            }),
        };
    }
    if editor.quick_export.is_some() {
        popover(editor, &response);
    }
}

#[derive(Default)]
struct Chosen {
    export: bool,
    more: bool,
    queue: bool,
    close: bool,
}

fn popover(editor: &mut Editor, button: &egui::Response) {
    let sequence = editor.timeline.sequence();
    let size = (sequence.format.width, sequence.format.height);
    let empty = sequence.duration() == timeline::Ticks::ZERO;
    let offered = if size.1 > size.0 {
        &TALL[..]
    } else {
        &WIDE[..]
    };
    let export_queue = &editor.export_queue;
    let busy = export_queue.progress().is_some();
    let running = editor.export.as_ref().and_then(ExportDialog::running_path);
    let sequence = editor.timeline.sequence();
    let Some(quick) = &mut editor.quick_export else {
        return;
    };
    let mut open = true;
    let mut chosen = Chosen::default();
    egui::Popup::from_response(button)
        .open_bool(&mut open)
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .width(340.0)
        .show(|ui| {
            let export = &mut quick.export;
            ui.label(RichText::new("Quick Export").strong());
            ui.add_space(4.0);
            ui.label("File");
            let field = egui::TextEdit::singleline(&mut export.path).desired_width(f32::INFINITY);
            ui.add(field);
            let target = export.target(sequence, || {
                let queued = export_queue.pending_paths().into_iter();
                queued.chain(running.map(Path::to_owned)).collect()
            });
            if let Some(Target::Taken { by, .. }) = target {
                let name = quoted_name(&export.output());
                let warning = match by {
                    Holder::File => format!("{name} will be replaced"),
                    Holder::Sidecar => {
                        let captions = sidecar_path(&export.output());
                        format!("{} will be replaced", quoted_name(&captions))
                    }
                    Holder::Queue => format!("Another export already writes {name}"),
                };
                ui.colored_label(ui.visuals().warn_fg_color, warning);
            }
            ui.add_space(4.0);
            ui.label("Preset");
            let kind = export.format.format;
            for name in offered {
                let Some(preset) = PRESETS.iter().find(|preset| preset.name == *name) else {
                    continue;
                };
                let current = export.format.preset_name() == Some(preset.name);
                if ui.radio(current, preset.name).clicked() {
                    export.format = ExportFormat {
                        acceleration: export.format.acceleration,
                        loudness: export.format.loudness,
                        ..preset.format
                    };
                }
            }
            if export.format.format != kind {
                export.follow_format();
            }
            ui.label(RichText::new(summary(&export.format, size)).weak());
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                chosen.more = ui
                    .link("More Settings…")
                    .on_hover_text("Open Export Media with these choices")
                    .clicked();
                if busy {
                    chosen.queue = ui.link("Export Queue").clicked();
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    chosen.export = ui
                        .add_enabled(!empty, crate::dialog::primary("Export"))
                        .on_disabled_hover_text("The sequence is empty")
                        .clicked();
                });
            });
            chosen.close = ui.input(|input| input.key_pressed(Key::Escape));
        });
    if chosen.export {
        start(editor);
    } else if chosen.more {
        if let Some(quick) = editor.quick_export.take() {
            editor.export.get_or_insert(quick.export);
        }
    } else if chosen.queue {
        editor.export_queue.open = true;
        editor.quick_export = None;
    } else if chosen.close || !open {
        editor.quick_export = None;
    }
}

/// Hands the export to the queue and starts it, closing the popover.
fn start(editor: &mut Editor) {
    let Some(quick) = editor.quick_export.take() else {
        return;
    };
    let Some((sequence, settings)) = export::prepare(editor, &quick.export) else {
        editor.quick_export = Some(quick);
        return;
    };
    let size = (sequence.format.width, sequence.format.height);
    let name = quoted_name(&settings.path);
    editor
        .export_queue
        .add(sequence, settings, summary(&quick.export.format, size));
    editor.export_queue.start();
    editor.notice(format!("Exporting {name} in the background"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_offered_preset_exists() {
        for name in WIDE.iter().chain(&TALL) {
            assert!(PRESETS.iter().any(|preset| preset.name == *name), "{name}");
        }
    }
}
