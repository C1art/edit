use std::ops::Range;

use egui::{Align2, Color32, DragAndDrop, FontId, Rect, Stroke, StrokeKind};
use timeline::{ClipId, PlaceMode, ReplaceClip, SourceId, Ticks};

use crate::editor::Editor;
use crate::placing;
use crate::theme;
use crate::timeline_view::SourceDrag;

/// Where media dropped on the program monitor goes, as Premiere's drop zones offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Zone {
    Insert,
    Overwrite,
    Overlay,
    Replace,
    InsertBefore,
    InsertAfter,
    Append,
}

impl Zone {
    const ROWS: [&'static [Zone]; 2] = [
        &[Zone::Insert, Zone::Overwrite, Zone::Overlay, Zone::Replace],
        &[Zone::InsertBefore, Zone::InsertAfter, Zone::Append],
    ];

    fn label(self) -> &'static str {
        match self {
            Zone::Insert => "Insert",
            Zone::Overwrite => "Overwrite",
            Zone::Overlay => "Overlay",
            Zone::Replace => "Replace",
            Zone::InsertBefore => "Insert Before",
            Zone::InsertAfter => "Insert After",
            Zone::Append => "Append at End",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Zone::Insert => "at the playhead, pushing later clips on",
            Zone::Overwrite => "at the playhead, over what's there",
            Zone::Overlay => "on the first free track above",
            Zone::Replace => "the clip under the playhead",
            Zone::InsertBefore => "the clip under the playhead",
            Zone::InsertAfter => "the clip under the playhead",
            Zone::Append => "after the last clip",
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
            Color32::from_black_alpha(200)
        };
        painter.rect_filled(area, 6.0, fill);
        let stroke = Stroke::new(1.0, Color32::from_white_alpha(if over { 200 } else { 60 }));
        painter.rect_stroke(area, 6.0, stroke, StrokeKind::Inside);
        painter.text(
            area.center(),
            Align2::CENTER_BOTTOM,
            zone.label(),
            FontId::proportional(14.0),
            theme::TEXT,
        );
        if area.height() > 44.0 {
            let hint = painter.layout(
                zone.hint().to_owned(),
                FontId::proportional(10.5),
                Color32::from_white_alpha(160),
                area.width() - 12.0,
            );
            let top = area.center() + egui::vec2(-hint.size().x / 2.0, 3.0);
            painter.galley(top, hint, Color32::WHITE);
        }
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
        Zone::Overlay => return placing::overlay_carried(editor, &media),
        Zone::Replace => return replace(editor, &media),
        Zone::InsertBefore | Zone::InsertAfter => {
            let Some((_, span)) = targeted_clip(editor) else {
                editor.notice("Put the playhead over a clip on a targeted track");
                return;
            };
            let at = if zone == Zone::InsertBefore {
                span.start
            } else {
                span.end
            };
            (at, PlaceMode::Insert)
        }
    };
    placing::place_carried(editor, &media, at, mode);
}

/// Puts one dragged item under the targeted clip at the playhead, from its in point.
fn replace(editor: &mut Editor, media: &[SourceDrag]) {
    let [drag] = media else {
        editor.notice("Replace takes a single item");
        return;
    };
    let Some((clip, _)) = targeted_clip(editor) else {
        editor.notice("Put the playhead over a clip on a targeted track");
        return;
    };
    editor.apply(ReplaceClip {
        clip,
        source: drag.source,
        source_in: drag.range.0,
    });
}

/// The clip under the playhead on the targeted video track, else the targeted audio.
fn targeted_clip(editor: &Editor) -> Option<(ClipId, Range<Ticks>)> {
    let sequence = editor.timeline.sequence();
    let playhead = editor.playhead();
    let targets = editor.view.targets;
    let video = targets.video.and_then(|index| sequence.video.get(index));
    let audio = targets.audio.and_then(|index| sequence.audio.get(index));
    video
        .into_iter()
        .chain(audio)
        .find_map(|track| track.clip_at(playhead))
        .map(|clip| (clip.id, clip.start..clip.end()))
}

/// Rows of zones across the middle of the picture, clear of its edges.
fn zones(rect: Rect) -> impl Iterator<Item = (Zone, Rect)> {
    let inner = rect.shrink2(egui::vec2(16.0, (rect.height() * 0.18).max(16.0)));
    let gap = 8.0;
    let height = (inner.height() - gap) / Zone::ROWS.len() as f32;
    Zone::ROWS
        .into_iter()
        .enumerate()
        .flat_map(move |(row, zones)| {
            let top = inner.top() + row as f32 * (height + gap);
            let count = zones.len() as f32;
            let width = (inner.width() - gap * (count - 1.0)) / count;
            zones.iter().enumerate().map(move |(index, &zone)| {
                let left = inner.left() + index as f32 * (width + gap);
                let area = Rect::from_x_y_ranges(left..=left + width, top..=top + height);
                (zone, area)
            })
        })
}

fn pointer_in(ctx: &egui::Context, rect: Rect) -> Option<egui::Pos2> {
    ctx.pointer_latest_pos().filter(|pos| rect.contains(*pos))
}

fn released(ctx: &egui::Context) -> bool {
    ctx.input(|input| input.pointer.any_released())
}
