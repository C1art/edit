mod clip_art;
mod gesture;
mod header;
mod interact;
mod layout;
mod markers;
mod paint;
mod rubber_band;
mod scale;
mod tool;
mod track_heights;
mod track_select;

use std::collections::BTreeSet;

use egui::{Rect, Sense};
use timeline::{
    ClipId, MarkerId, MarkerOwner, Sequence, SetTrackSwitch, SourceId, Ticks, Timeline, TrackKind,
    TrackRef,
};

use crate::clip_art::ClipArt;
use gesture::{Gesture, PendingEdit};
use header::HeaderAction;
pub use markers::color as marker_color;
pub use paint::label_color;
use scale::TimeScale;
pub use tool::{Direction, Tool};
pub use track_heights::TrackHeights;

const HEADER_WIDTH: f32 = 250.0;
const RULER_HEIGHT: f32 = 26.0;
/// Snapping reach in points, so it feels the same at every zoom level.
const SNAP_POINTS: f32 = 8.0;
/// About ten seconds across a typical timeline panel.
const DEFAULT_TICKS_PER_POINT: f64 = 900.0;

/// The tracks that Insert and Overwrite place source video and audio on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Targets {
    pub video: Option<usize>,
    pub audio: Option<usize>,
}

impl Default for Targets {
    fn default() -> Self {
        Self {
            video: Some(0),
            audio: Some(0),
        }
    }
}

impl Targets {
    fn contains(&self, track: TrackRef) -> bool {
        let target = match track.kind {
            TrackKind::Video => self.video,
            TrackKind::Audio => self.audio,
        };
        target == Some(track.index)
    }

    fn toggle(&mut self, track: TrackRef) {
        let target = match track.kind {
            TrackKind::Video => &mut self.video,
            TrackKind::Audio => &mut self.audio,
        };
        *target = if *target == Some(track.index) {
            None
        } else {
            Some(track.index)
        };
    }
}

/// Media dropped from the pool onto a track.
#[derive(Debug, Clone, Copy)]
pub struct MediaDrop {
    pub source: SourceId,
    pub track: TrackRef,
    pub at: Ticks,
}

#[derive(Debug, Default)]
pub struct TimelineOutput {
    pub seek: Option<Ticks>,
    /// A marker to open in the marker dialog.
    pub edit_marker: Option<(MarkerOwner, MarkerId)>,
    pub clear_marker: Option<(MarkerOwner, MarkerId)>,
    pub drop: Option<MediaDrop>,
    /// A double-clicked clip, whose media to open.
    pub open_clip: Option<ClipId>,
    /// The clip area while a right-click on a clip has its menu open.
    pub clip_menu: Option<egui::Response>,
    pub errors: Vec<String>,
}

struct Drag {
    gesture: Gesture,
    origin: Ticks,
    pending: Option<PendingEdit>,
    preview: Option<Sequence>,
}

pub struct TimelineView {
    pub tool: Tool,
    pub snapping: bool,
    pub selection: Vec<ClipId>,
    pub targets: Targets,
    scroll: Ticks,
    scroll_y: f32,
    ticks_per_point: f64,
    /// Width of the clip area at the last layout, for zoom-to-fit.
    width: f32,
    drag: Option<Drag>,
    /// The marker a ruler context menu was opened on.
    context_marker: Option<MarkerId>,
    /// Whether the last right-click in the clip area landed on a clip.
    context_clip: bool,
    /// Audio tracks showing their automation in place of their clips' volume lines.
    automation_shown: BTreeSet<TrackRef>,
    pub heights: TrackHeights,
    /// The costly segments, each with whether a render preview of it is ready.
    pub render_bar: Vec<(std::ops::Range<Ticks>, bool)>,
}

impl Default for TimelineView {
    fn default() -> Self {
        Self {
            tool: Tool::default(),
            snapping: true,
            selection: Vec::new(),
            targets: Targets::default(),
            scroll: Ticks::ZERO,
            scroll_y: 0.0,
            ticks_per_point: DEFAULT_TICKS_PER_POINT,
            width: 1000.0,
            drag: None,
            context_marker: None,
            context_clip: false,
            automation_shown: BTreeSet::new(),
            heights: TrackHeights::default(),
            render_bar: Vec::new(),
        }
    }
}

impl TimelineView {
    /// Zooms by `factor` (above one zooms in) keeping time at screen x `anchor` fixed.
    pub fn zoom(&mut self, factor: f32, anchor: Option<f32>) {
        let anchor = anchor.unwrap_or(self.width / 2.0);
        let scale = self.scale(0.0);
        let fixed = scale.time(anchor);
        self.ticks_per_point = (self.ticks_per_point / f64::from(factor)).clamp(1.0, 1.0e6);
        self.scroll = (fixed - self.scale(0.0).span(anchor)).max(Ticks::ZERO);
    }

    pub fn zoom_to_fit(&mut self, sequence: &Sequence) {
        let duration = sequence.duration().max(Ticks(
            i64::from(sequence.timebase.den()) / i64::from(sequence.timebase.num()),
        ));
        self.ticks_per_point = (duration.0 as f64 / f64::from(self.width.max(1.0)) * 1.05).max(1.0);
        self.scroll = Ticks::ZERO;
    }

    /// Drops selected clips that no longer exist, e.g. after undo.
    pub fn prune_selection(&mut self, sequence: &Sequence) {
        self.selection.retain(|&id| sequence.clip(id).is_some());
        let video = sequence.video.len();
        let audio = sequence.audio.len();
        self.targets.video = self.targets.video.filter(|&index| index < video);
        self.targets.audio = self.targets.audio.filter(|&index| index < audio);
    }

    pub fn cancel_drag(&mut self) {
        self.drag = None;
    }

    fn scale(&self, left: f32) -> TimeScale {
        TimeScale {
            left,
            scroll: self.scroll,
            ticks_per_point: self.ticks_per_point,
        }
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        timeline: &mut Timeline,
        art: &mut ClipArt,
        playhead: Ticks,
        playing: bool,
    ) -> TimelineOutput {
        let mut output = TimelineOutput::default();
        let area = ui.available_rect_before_wrap();
        ui.allocate_rect(area, Sense::hover());
        let headers = egui::Rangef::new(area.left(), area.left() + HEADER_WIDTH);
        let ruler = Rect::from_x_y_ranges(
            headers.max..=area.right(),
            area.top()..=area.top() + RULER_HEIGHT,
        );
        let tracks = Rect::from_x_y_ranges(ruler.x_range(), ruler.bottom()..=area.bottom());
        self.width = tracks.width();

        self.handle_scroll(ui, area, tracks, timeline.sequence());
        if playing {
            self.follow(playhead, tracks.width());
        }
        let scale = self.scale(tracks.left());

        let ruler_response =
            ui.interact(ruler, ui.scope_id().with("ruler"), Sense::click_and_drag());
        self.ruler_markers(&ruler_response, timeline.sequence(), &scale, &mut output);
        if let Some(pointer) = ruler_response.interact_pointer_pos() {
            let sequence = timeline.sequence();
            let marker = markers::hit(&scale, sequence, pointer.x)
                .filter(|_| !ruler_response.dragged())
                .and_then(|id| sequence.markers.iter().find(|marker| marker.id == id));
            let time = marker.map_or_else(
                || {
                    scale::nearest_frame(
                        scale.time(pointer.x).max(Ticks::ZERO),
                        sequence.format.frame_rate,
                        sequence.timebase,
                    )
                },
                |marker| marker.at,
            );
            output.seek = Some(time);
        }

        let response = ui.interact(
            tracks,
            ui.scope_id().with("tracks"),
            Sense::click_and_drag(),
        );
        let top = tracks.top() - self.scroll_y;
        self.handle_gestures(&response, timeline, scale, top, playhead, &mut output);
        output.clip_menu = self.context_clip.then(|| response.clone());
        let drop_guide = handle_drop(
            &response,
            timeline.sequence(),
            &self.heights,
            scale,
            top,
            &mut output,
        );

        let sequence = self
            .drag
            .as_ref()
            .and_then(|drag| drag.preview.as_ref())
            .unwrap_or(timeline.sequence());
        let rows = layout::rows(sequence, &self.heights, top);
        let painter = ui.painter_at(tracks);
        for row in &rows {
            paint::row_background(
                &painter,
                tracks,
                row,
                sequence,
                self.targets.contains(row.track),
            );
            let automation = self.automation_shown.contains(&row.track);
            paint::clips(
                &painter,
                row,
                &scale,
                sequence,
                &self.selection,
                art,
                !automation,
            );
            if let Some(track) = sequence.track(row.track).filter(|_| automation) {
                let end = scale.time(tracks.right());
                let rect = layout::clip_rect(row, &scale, Ticks::ZERO, end).shrink(1.0);
                let envelope = rubber_band::Envelope::track(track, end);
                rubber_band::paint(&painter, rect, &scale, &envelope);
            }
        }
        if let Some(at) = drop_guide {
            paint::guide(&painter, scale.x(at), tracks.top(), tracks.bottom());
        }
        paint::ruler(&ui.painter_at(ruler), ruler, &scale, sequence);
        paint::render_bar(&ui.painter_at(ruler), ruler, &scale, &self.render_bar);
        markers::paint_in_out(
            &ui.painter_at(ruler.union(tracks)),
            ruler,
            tracks.bottom(),
            &scale,
            sequence,
        );
        markers::paint(&ui.painter_at(ruler), ruler, &scale, sequence);
        paint::playhead(
            &ui.painter_at(ruler.union(tracks)),
            ruler,
            tracks.bottom(),
            &scale,
            playhead,
        );

        let header_column = Rect::from_x_y_ranges(headers, tracks.y_range());
        let mut header_ui = ui.new_child(egui::UiBuilder::new().max_rect(header_column));
        header_ui.set_clip_rect(header_column);
        let actions: Vec<HeaderAction> = rows
            .iter()
            .filter_map(|row| {
                let automation = self.automation_shown.contains(&row.track);
                header::header(
                    &mut header_ui,
                    headers,
                    row,
                    sequence,
                    &self.targets,
                    automation,
                )
            })
            .collect();
        track_heights::resize_grips(&mut self.heights, &header_ui, &rows, header_column);
        for action in actions {
            match action {
                HeaderAction::ToggleTarget(track) => self.targets.toggle(track),
                HeaderAction::ToggleAutomation(track) => {
                    if !self.automation_shown.remove(&track) {
                        self.automation_shown.insert(track);
                    }
                }
                HeaderAction::Switch(track, switch, on) => {
                    if let Err(error) = timeline.apply(SetTrackSwitch { track, switch, on }) {
                        output.errors.push(error.to_string());
                    }
                }
            }
        }
        output
    }

    /// Double-click opens a marker; right-click offers to edit or clear it.
    fn ruler_markers(
        &mut self,
        response: &egui::Response,
        sequence: &Sequence,
        scale: &TimeScale,
        output: &mut TimelineOutput,
    ) {
        let hit =
            |pos: Option<egui::Pos2>| pos.and_then(|pos| markers::hit(scale, sequence, pos.x));
        if response.double_clicked() {
            output.edit_marker =
                hit(response.interact_pointer_pos()).map(|id| (MarkerOwner::Sequence, id));
        }
        if response.secondary_clicked() {
            self.context_marker = hit(response.interact_pointer_pos());
        }
        let Some(id) = self.context_marker else {
            return;
        };
        response.context_menu(|ui| {
            if ui.button("Edit Marker…").clicked() {
                output.edit_marker = Some((MarkerOwner::Sequence, id));
            }
            if ui.button("Clear Marker").clicked() {
                output.clear_marker = Some((MarkerOwner::Sequence, id));
            }
        });
    }

    fn handle_scroll(&mut self, ui: &egui::Ui, area: Rect, tracks: Rect, sequence: &Sequence) {
        if !ui.rect_contains_pointer(area) {
            return;
        }
        let (scroll, zoom, pointer) = ui.input(|input| {
            (
                input.smooth_scroll_delta,
                input.zoom_delta(),
                input.pointer.hover_pos(),
            )
        });
        if zoom != 1.0 {
            self.zoom(zoom, pointer.map(|pos| pos.x - tracks.left()));
        }
        if scroll.x != 0.0 {
            let scale = self.scale(0.0);
            self.scroll = (self.scroll - scale.span(scroll.x)).max(Ticks::ZERO);
        }
        if scroll.y != 0.0 {
            let overflow =
                (layout::total_height(sequence, &self.heights) - tracks.height() + 20.0).max(0.0);
            self.scroll_y = (self.scroll_y - scroll.y).clamp(0.0, overflow);
        }
    }

    /// Pages the view along so the playhead stays visible during playback.
    fn follow(&mut self, playhead: Ticks, width: f32) {
        let scale = self.scale(0.0);
        let x = scale.x(playhead);
        if x > width - 10.0 || x < 0.0 {
            self.scroll = (playhead - scale.span(width * 0.1)).max(Ticks::ZERO);
        }
    }
}

/// Reports a pool item released over a track and returns where a hovered one would land.
fn handle_drop(
    response: &egui::Response,
    sequence: &Sequence,
    heights: &TrackHeights,
    scale: TimeScale,
    top: f32,
    output: &mut TimelineOutput,
) -> Option<Ticks> {
    let pos = response.hover_pos()?;
    let at = scale::nearest_frame(
        scale.time(pos.x).max(Ticks::ZERO),
        sequence.format.frame_rate,
        sequence.timebase,
    );
    let rows = layout::rows(sequence, heights, top);
    if let (Some(source), Some(row)) = (
        response.dnd_release_payload::<SourceId>(),
        layout::row_at(&rows, pos.y),
    ) {
        output.drop = Some(MediaDrop {
            source: *source,
            track: row.track,
            at,
        });
    }
    response.dnd_hover_payload::<SourceId>().map(|_| at)
}
