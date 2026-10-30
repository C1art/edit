use egui::{RichText, Sense, Stroke};
use timeline::{MarkerColor, MarkerId, MarkerKind, MarkerOwner, Ticks, Timecode};

use crate::editor::Editor;
use crate::timeline_view::marker_color;

const SWATCH_RADIUS: f32 = 5.0;

/// What the marker list shows: colors toggled off and text to search names and
/// comments for.
#[derive(Debug, Default)]
pub struct MarkerFilter {
    hidden: Vec<MarkerColor>,
    search: String,
}

enum RowAction {
    Seek(Ticks),
    Edit(MarkerId),
}

/// Every sequence marker in time order with its color, timecode, name and comment.
/// Clicking one seeks to it and double-clicking opens it for editing.
pub fn ui(editor: &mut Editor, ui: &mut egui::Ui) {
    let playhead = editor.playhead();
    let filter = &mut editor.marker_filter;
    ui.horizontal_wrapped(|ui| {
        for color in MarkerColor::ALL {
            let shown = !filter.hidden.contains(&color);
            let (rect, response) =
                ui.allocate_exact_size(egui::Vec2::splat(SWATCH_RADIUS * 3.0), Sense::click());
            let fill = marker_color(color);
            let fill = if shown {
                fill
            } else {
                fill.gamma_multiply(0.2)
            };
            ui.painter()
                .circle_filled(rect.center(), SWATCH_RADIUS, fill);
            let verb = if shown { "Hide" } else { "Show" };
            if response
                .on_hover_text(format!("{verb} {} markers", color.name()))
                .clicked()
            {
                if shown {
                    filter.hidden.push(color);
                } else {
                    filter.hidden.retain(|&hidden| hidden != color);
                }
            }
        }
    });
    ui.add(
        egui::TextEdit::singleline(&mut filter.search)
            .hint_text("Search")
            .desired_width(f32::INFINITY),
    );
    let search = filter.search.to_lowercase();
    let sequence = editor.timeline.sequence();
    let (timebase, rate, display) = (
        sequence.timebase,
        sequence.format.frame_rate,
        sequence.time_display,
    );
    let mut action = None;
    egui::ScrollArea::vertical()
        .auto_shrink([false, true])
        .show(ui, |ui| {
            let shown = sequence.markers.iter().filter(|marker| {
                !filter.hidden.contains(&marker.color)
                    && (search.is_empty()
                        || marker.name.to_lowercase().contains(&search)
                        || marker.comment.to_lowercase().contains(&search))
            });
            for marker in shown {
                let row = ui.horizontal(|ui| {
                    let (swatch, _) = ui.allocate_exact_size(
                        egui::Vec2::splat(SWATCH_RADIUS * 2.0 + 4.0),
                        Sense::hover(),
                    );
                    ui.painter().circle(
                        swatch.center(),
                        SWATCH_RADIUS,
                        marker_color(marker.color),
                        Stroke::new(1.0, egui::Color32::from_black_alpha(160)),
                    );
                    let mut timecode =
                        Timecode::shown(marker.at, timebase, rate, display).to_string();
                    if marker.duration > Ticks::ZERO {
                        let length = Timecode::shown(marker.duration, timebase, rate, display);
                        timecode.push_str(&format!(" · {length}"));
                    }
                    ui.vertical(|ui| {
                        let name = if marker.name.is_empty() {
                            RichText::new("Unnamed").weak()
                        } else {
                            RichText::new(&marker.name).strong()
                        };
                        let label = ui.selectable_label(
                            marker.at == playhead,
                            RichText::new(timecode).monospace(),
                        );
                        ui.label(name);
                        if marker.kind != MarkerKind::Comment {
                            ui.label(RichText::new(marker.kind.name()).small());
                        }
                        if !marker.comment.is_empty() {
                            ui.label(RichText::new(&marker.comment).small().weak());
                        }
                        label
                    })
                    .inner
                });
                let label = row.inner;
                let full_width = row.response.rect.with_max_x(ui.max_rect().right());
                let whole = ui.interact(
                    full_width,
                    ui.make_persistent_id(("marker_row", marker.id.0)),
                    Sense::click(),
                );
                if label.double_clicked() || whole.double_clicked() {
                    action = Some(RowAction::Edit(marker.id));
                } else if label.clicked() || whole.clicked() {
                    action = Some(RowAction::Seek(marker.at));
                }
                ui.separator();
            }
            if sequence.markers.is_empty() {
                ui.label(RichText::new("Press M to add a marker at the playhead.").weak());
            }
        });
    match action {
        Some(RowAction::Seek(at)) => editor.seek_program(at),
        Some(RowAction::Edit(id)) => editor.edit_marker((MarkerOwner::Sequence, id)),
        None => {}
    }
}
