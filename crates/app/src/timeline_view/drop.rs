use egui::{Color32, Painter, Rect, Stroke, StrokeKind};
use timeline::{nearest_transition_edge, ClipId, Edge, Sequence, SourceId, Ticks, TrackRef};

use super::layout::{self, Row};
use super::scale::{self, TimeScale};
use super::{paint, TimelineOutput, TrackHeights};
use crate::effects_panel::{EffectDrop, EffectItem};

/// Media dropped from the pool onto a track.
#[derive(Debug, Clone, Copy)]
pub struct MediaDrop {
    pub source: SourceId,
    pub track: TrackRef,
    pub at: Ticks,
}

/// Where something dragged over the tracks would land.
#[derive(Debug, Clone, Copy)]
pub(super) enum DropHint {
    At(Ticks),
    On(TrackRef, ClipId),
}

/// Reports a pool item or effect released over a track, and returns where a hovered one
/// would land.
pub(super) fn handle_drop(
    response: &egui::Response,
    sequence: &Sequence,
    heights: &TrackHeights,
    scale: TimeScale,
    top: f32,
    output: &mut TimelineOutput,
) -> Option<DropHint> {
    // `hover_pos` is unset while the drag that brings a payload holds the pointer.
    let pos = response
        .ctx
        .pointer_hover_pos()
        .filter(|_| response.contains_pointer())?;
    let time = scale.time(pos.x).max(Ticks::ZERO);
    let rows = layout::rows(sequence, heights, top);
    let row = layout::row_at(&rows, pos.y);
    if response.dnd_hover_payload::<SourceId>().is_some() {
        let at = scale::nearest_frame(time, sequence.format.frame_rate, sequence.timebase);
        if let (Some(source), Some(row)) = (response.dnd_release_payload::<SourceId>(), row) {
            output.drop = Some(MediaDrop {
                source: *source,
                track: row.track,
                at,
            });
        }
        return Some(DropHint::At(at));
    }
    let item = response.dnd_hover_payload::<EffectItem>()?;
    let track = row?.track;
    if let Some(item) = response.dnd_release_payload::<EffectItem>() {
        output.effect_drop = Some(EffectDrop {
            item: (*item).clone(),
            track,
            at: time,
        });
    }
    if item.track_kind().is_some_and(|kind| kind != track.kind) {
        return None;
    }
    let clips = sequence.track(track)?;
    match *item {
        EffectItem::Transition(..) => {
            let (id, edge) = nearest_transition_edge(clips, time)?;
            let (_, clip) = sequence.clip(id)?;
            let at = match edge {
                Edge::Head => clip.start,
                Edge::Tail => clip.end(),
            };
            Some(DropHint::At(at))
        }
        EffectItem::Video(_) | EffectItem::Audio(_) | EffectItem::Preset(_) => {
            clips.clip_at(time).map(|clip| DropHint::On(track, clip.id))
        }
    }
}

pub(super) fn paint(
    painter: &Painter,
    hint: DropHint,
    rows: &[Row],
    scale: &TimeScale,
    sequence: &Sequence,
    tracks: Rect,
) {
    match hint {
        DropHint::At(at) => paint::guide(painter, scale.x(at), tracks.top(), tracks.bottom()),
        DropHint::On(track, id) => {
            let row = rows.iter().find(|row| row.track == track);
            if let (Some(row), Some((_, clip))) = (row, sequence.clip(id)) {
                let rect = layout::clip_rect(row, scale, clip.start, clip.end());
                let stroke = Stroke::new(2.0, Color32::from_rgb(250, 210, 60));
                painter.rect_stroke(rect, 2.0, stroke, StrokeKind::Inside);
            }
        }
    }
}

/// What an empty sequence shows, so a first-time user knows how to start.
pub(super) fn paint_empty(painter: &Painter, tracks: Rect) {
    let galley = painter.layout_no_wrap(
        "Drag clips here from the Project panel or the Source monitor".into(),
        egui::FontId::proportional(14.0),
        crate::theme::WEAK_TEXT,
    );
    let text = Rect::from_center_size(tracks.center(), galley.size());
    painter.rect_filled(text.expand2(egui::vec2(14.0, 7.0)), 8.0, crate::theme::WELL);
    painter.galley(text.min, galley, crate::theme::WEAK_TEXT);
}
