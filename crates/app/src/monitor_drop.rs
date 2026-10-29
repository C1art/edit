use egui::{Align2, Color32, DragAndDrop, FontId, Rect, Stroke, StrokeKind};
use timeline::{PlaceMode, SourceId};

use crate::editor::Editor;
use crate::placing;
use crate::theme;
use crate::timeline_view::SourceDrag;

/// Where media dropped on the program monitor goes, as Premiere's drop zones offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Zone {
    Insert,
    Overwrite,
    Append,
}

impl Zone {
    const ALL: [Zone; 3] = [Zone::Insert, Zone::Overwrite, Zone::Append];

    fn label(self) -> &'static str {
        match self {
            Zone::Insert => "Insert",
            Zone::Overwrite => "Overwrite",
            Zone::Append => "Append at End",
        }
    }
}

/// Opens an item dragged from the project and dropped on the source monitor.
pub fn source(editor: &mut Editor, ui: &egui::Ui, rect: Rect) {
    let ctx = ui.ctx().clone();
    if !DragAndDrop::has_payload_of_type::<SourceId>(&ctx) || pointer_in(&ctx, rect).is_none() {
        return;
    }
    let stroke = Stroke::new(2.0, theme::ACCENT);
    ui.painter()
        .rect_stroke(rect, 2.0, stroke, StrokeKind::Inside);
    if released(&ctx) {
        if let Some(id) = DragAndDrop::take_payload::<SourceId>(&ctx) {
            editor.open_source(*id);
        }
    }
}

/// Offers zones over the program picture for media carried from the project or the
/// source monitor, editing it in at the playhead or after the end of the sequence.
pub fn program(editor: &mut Editor, ui: &egui::Ui, rect: Rect) {
    let ctx = ui.ctx().clone();
    let from_pool = DragAndDrop::has_payload_of_type::<SourceId>(&ctx);
    if !from_pool && !DragAndDrop::has_payload_of_type::<SourceDrag>(&ctx) {
        return;
    }
    let Some(pointer) = pointer_in(&ctx, rect) else {
        return;
    };
    let painter = ui.painter_at(rect);
    let mut hovered = None;
    for (zone, area) in zones(rect) {
        let over = area.contains(pointer);
        if over {
            hovered = Some(zone);
        }
        let fill = if over {
            theme::ACCENT_FILL.gamma_multiply(0.85)
        } else {
            Color32::from_black_alpha(170)
        };
        painter.rect_filled(area, 6.0, fill);
        let stroke = Stroke::new(1.0, Color32::from_white_alpha(if over { 200 } else { 60 }));
        painter.rect_stroke(area, 6.0, stroke, StrokeKind::Inside);
        painter.text(
            area.center(),
            Align2::CENTER_CENTER,
            zone.label(),
            FontId::proportional(14.0),
            theme::TEXT,
        );
    }
    let Some(zone) = hovered.filter(|_| released(&ctx)) else {
        return;
    };
    let media = placing::carried_media(editor, &ctx);
    // Taking a payload of the wrong type would discard it.
    if from_pool {
        DragAndDrop::take_payload::<SourceId>(&ctx);
    } else {
        DragAndDrop::take_payload::<SourceDrag>(&ctx);
    }
    let playhead = editor.playhead();
    let (at, mode) = match zone {
        Zone::Insert => (playhead, PlaceMode::Insert),
        Zone::Overwrite => (playhead, PlaceMode::Overwrite),
        Zone::Append => (editor.timeline.sequence().duration(), PlaceMode::Overwrite),
    };
    placing::place_carried(editor, &media, at, mode);
}

/// Three columns across the middle of the picture, clear of its edges.
fn zones(rect: Rect) -> impl Iterator<Item = (Zone, Rect)> {
    let inner = rect.shrink2(egui::vec2(16.0, (rect.height() * 0.25).max(16.0)));
    let gap = 8.0;
    let width = (inner.width() - gap * 2.0) / 3.0;
    Zone::ALL.into_iter().enumerate().map(move |(index, zone)| {
        let left = inner.left() + index as f32 * (width + gap);
        let area = Rect::from_x_y_ranges(left..=left + width, inner.y_range());
        (zone, area)
    })
}

fn pointer_in(ctx: &egui::Context, rect: Rect) -> Option<egui::Pos2> {
    ctx.pointer_latest_pos().filter(|pos| rect.contains(*pos))
}

fn released(ctx: &egui::Context) -> bool {
    ctx.input(|input| input.pointer.any_released())
}
