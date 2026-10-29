use egui::{Color32, Painter, Rect, Stroke, StrokeKind};
use timeline::{
    nearest_transition_edge, ClipId, Edge, MediaItem, PlaceMode, Sequence, SourceId, Ticks,
    TrackKind, TrackRef,
};

use super::layout::{self, Row};
use super::scale::{self, TimeScale};
use super::{paint, TimelineOutput, SNAP_POINTS};
use crate::effects_panel::{EffectDrop, EffectItem};

/// Media dropped from the pool or the source monitor onto a track, laid end to end
/// when several items were dragged.
#[derive(Debug, Clone)]
pub struct MediaDrop {
    pub media: Vec<SourceDrag>,
    pub track: TrackRef,
    pub at: Ticks,
    pub mode: PlaceMode,
}

/// Media dragged from the pool or the source monitor, with the part of it a drop
/// places as `(source_in, duration)` in the media's time.
#[derive(Debug, Clone, Copy)]
pub struct SourceDrag {
    pub source: SourceId,
    pub range: (Ticks, Ticks),
    pub take: Take,
}

/// Which of a dragged item's picture and sound to place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Take {
    Both,
    VideoOnly,
    AudioOnly,
}

/// The video and audio track indexes media dropped on `track` lands on: that one and
/// the first of the other kind, as far as any of the media and its take have picture
/// and sound.
pub fn drop_tracks(
    sequence: &Sequence,
    media: &[SourceDrag],
    track: TrackRef,
) -> (Option<usize>, Option<usize>) {
    let first = |count: usize| (count > 0).then_some(0);
    let (video, audio) = match track.kind {
        TrackKind::Video => (Some(track.index), first(sequence.audio.len())),
        TrackKind::Audio => (first(sequence.video.len()), Some(track.index)),
    };
    let takes = |wanted: fn(&MediaItem, Take) -> bool| {
        media.iter().any(|drag| {
            sequence
                .media_item(drag.source)
                .is_some_and(|item| wanted(item, drag.take))
        })
    };
    (
        video.filter(|_| takes(|item, take| item.video.is_some() && take != Take::AudioOnly)),
        audio.filter(|_| takes(|item, take| item.has_audio && take != Take::VideoOnly)),
    )
}

/// Where something dragged over the tracks would land.
#[derive(Debug, Clone, Copy)]
pub(super) enum DropHint {
    At(Ticks),
    Media {
        start: Ticks,
        end: Ticks,
        tracks: [Option<TrackRef>; 2],
        mode: PlaceMode,
    },
    On(TrackRef, ClipId),
}

/// Reports media or an effect released over a track, and returns where a hovered one
/// would land. `carried` is the media being dragged, which snaps by its head and tail
/// when given the playhead to snap with too, and is inserted rather than laid over
/// what's there while Ctrl is held, as in Premiere.
pub(super) fn handle_drop(
    response: &egui::Response,
    sequence: &Sequence,
    rows: &[Row],
    scale: TimeScale,
    snapping_with: Option<Ticks>,
    carried: &[SourceDrag],
    output: &mut TimelineOutput,
) -> Option<DropHint> {
    // `hover_pos` is unset while the drag that brings a payload holds the pointer.
    let pos = response
        .ctx
        .pointer_hover_pos()
        .filter(|_| response.contains_pointer())?;
    let time = scale.time(pos.x).max(Ticks::ZERO);
    let row = layout::row_at(rows, pos.y);
    let from_pool = response.dnd_hover_payload::<SourceId>().is_some();
    let hovering = from_pool || response.dnd_hover_payload::<SourceDrag>().is_some();
    if hovering && !carried.is_empty() {
        let mut length = Ticks::ZERO;
        for drag in carried {
            let item = sequence.media_item(drag.source)?;
            length = length + item.conform().sequence_span(drag.range.1);
        }
        let mut at = scale::nearest_frame(time, sequence.format.frame_rate, sequence.timebase);
        if let Some(playhead) = snapping_with {
            let targets = scale::snap_targets(sequence, &[], playhead);
            let edges = [at, at + length];
            at = at + scale::snap(Ticks::ZERO, &edges, &targets, scale.span(SNAP_POINTS));
        }
        let Some(row) = row else {
            return Some(DropHint::At(at));
        };
        let mode = if response.ctx.input(|input| input.modifiers.command) {
            PlaceMode::Insert
        } else {
            PlaceMode::Overwrite
        };
        // Releasing takes the payload whatever its type, so only the hovered one may try.
        let released = if from_pool {
            response.dnd_release_payload::<SourceId>().is_some()
        } else {
            response.dnd_release_payload::<SourceDrag>().is_some()
        };
        if released {
            output.drop = Some(MediaDrop {
                media: carried.to_vec(),
                track: row.track,
                at,
                mode,
            });
        }
        let (video, audio) = drop_tracks(sequence, carried, row.track);
        return Some(DropHint::Media {
            start: at,
            end: at + length,
            tracks: [video.map(TrackRef::video), audio.map(TrackRef::audio)],
            mode,
        });
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
        DropHint::Media {
            start,
            end,
            tracks: on,
            mode,
        } => {
            for row in rows.iter().filter(|row| on.contains(&Some(row.track))) {
                let rect = layout::clip_rect(row, scale, start, end);
                painter.rect_filled(rect, 2.0, Color32::from_white_alpha(24));
                let stroke = Stroke::new(1.5, Color32::from_white_alpha(200));
                painter.rect_stroke(rect, 2.0, stroke, StrokeKind::Inside);
                if mode == PlaceMode::Insert {
                    paint_push(painter, rect.left_center(), stroke.color);
                }
            }
            paint::guide(painter, scale.x(start), tracks.top(), tracks.bottom());
        }
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

/// An arrow pushing to the right from `at`, for an insert that moves later clips along.
fn paint_push(painter: &Painter, at: egui::Pos2, color: Color32) {
    let arrow = [(6.0, -5.0), (12.0, 0.0), (6.0, 5.0)].map(|(x, y)| at + egui::vec2(x, y));
    painter.add(egui::Shape::convex_polygon(
        arrow.to_vec(),
        color,
        Stroke::NONE,
    ));
    let shaft = [at + egui::vec2(2.0, 0.0), at + egui::vec2(7.0, 0.0)];
    painter.line_segment(shaft, Stroke::new(2.0, color));
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
