mod clip_art;
mod clip_tip;
mod corner;
mod cursor;
mod drag_readout;
mod drop;
mod edge_scroll;
mod edit_point;
mod gesture;
mod header;
mod interact;
mod layout;
mod markers;
mod navigate;
mod paint;
mod rubber_band;
mod scale;
mod scrub;
mod time_scrollbar;
mod tool;
mod track_heights;
mod track_scroll;
mod track_select;
mod trim_view;

use std::collections::BTreeSet;

use egui::{Rect, Sense};
use timeline::{
    AddTrack, ClipId, DeltaBounds, LinkedSelection, MarkerId, MarkerOwner, RemoveTrack,
    RenameTrack, Sequence, SetTrackSwitch, Ticks, Timeline, TrackKind, TrackRef,
};

use crate::clip_art::ClipArt;
use crate::effects_panel::EffectDrop;
use drop::handle_drop;
pub use drop::MediaDrop;
pub use edit_point::{EditPoint, TrimType};
use gesture::{Gesture, PendingEdit};
use header::HeaderAction;
pub use markers::color as marker_color;
pub use paint::label_color;
use scale::TimeScale;
pub use tool::{Direction, Tool};
pub use track_heights::TrackHeights;
pub use trim_view::TrimView;

const HEADER_WIDTH: f32 = 200.0;
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

#[derive(Debug, Default)]
pub struct TimelineOutput {
    pub seek: Option<Ticks>,
    /// A marker to open in the marker dialog.
    pub edit_marker: Option<(MarkerOwner, MarkerId)>,
    pub clear_marker: Option<(MarkerOwner, MarkerId)>,
    pub drop: Option<MediaDrop>,
    pub effect_drop: Option<EffectDrop>,
    /// A double-clicked clip, whose media to open.
    pub open_clip: Option<ClipId>,
    /// The clip area while a right-click on a clip has its menu open.
    pub clip_menu: Option<egui::Response>,
    /// The clip area while a right-click on a gap has its menu open.
    pub gap_menu: Option<egui::Response>,
    pub errors: Vec<String>,
}

struct Drag {
    gesture: Gesture,
    /// `Apart` when the drag began with Alt held, so it leaves linked partners be.
    links: LinkedSelection,
    origin: Ticks,
    pending: Option<PendingEdit>,
    preview: Option<Sequence>,
    /// The sequence with linked partners left be, made once as an `Apart` drag starts.
    unlinked: Option<Sequence>,
    bounds: DeltaBounds,
    snapped_to: Option<Ticks>,
}

pub struct TimelineView {
    pub tool: Tool,
    pub snapping: bool,
    pub selection: Vec<ClipId>,
    /// A selected gap: its track and a time inside it.
    pub gap: Option<(TrackRef, Ticks)>,
    /// A cut selected for keyboard trimming.
    pub edit_point: Option<EditPoint>,
    pub targets: Targets,
    scroll: Ticks,
    scroll_y: f32,
    /// Whether the tracks have been scrolled by hand, ending their automatic placement.
    tracks_scrolled: bool,
    ticks_per_point: f64,
    /// Width of the clip area at the last layout, for zoom-to-fit.
    width: f32,
    drag: Option<Drag>,
    /// The marker a ruler context menu was opened on.
    context_marker: Option<MarkerId>,
    /// Whether the last right-click in the clip area landed on a clip.
    context_clip: bool,
    context_gap: bool,
    /// Audio tracks showing their automation in place of their clips' volume lines.
    automation_shown: BTreeSet<TrackRef>,
    renaming: Option<header::Renaming>,
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
            gap: None,
            edit_point: None,
            targets: Targets::default(),
            scroll: Ticks::ZERO,
            scroll_y: 0.0,
            tracks_scrolled: false,
            ticks_per_point: DEFAULT_TICKS_PER_POINT,
            width: 1000.0,
            drag: None,
            context_marker: None,
            context_clip: false,
            context_gap: false,
            automation_shown: BTreeSet::new(),
            renaming: None,
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
        self.gap = self
            .gap
            .filter(|&(track, at)| gap(sequence, track, at).is_some());
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
        let time_bar = Rect::from_x_y_ranges(
            ruler.x_range(),
            area.bottom() - time_scrollbar::HEIGHT..=area.bottom(),
        );
        let tracks = Rect::from_x_y_ranges(ruler.x_range(), ruler.bottom()..=time_bar.top());
        self.width = tracks.width();

        self.place_tracks(timeline.sequence(), tracks);
        self.handle_scroll(ui, area, tracks, timeline.sequence());
        let ruler_id = ui.scope_id().with("ruler");
        self.edge_scroll(ui, tracks, ruler_id);
        if playing {
            self.follow(playhead, tracks.width());
        }
        let scale = self.scale(tracks.left());

        let corner = Rect::from_x_y_ranges(headers, ruler.y_range());
        self.corner_timecode(ui, corner, timeline.sequence(), playhead, &mut output);
        let ruler_response = ui.interact(ruler, ruler_id, Sense::click_and_drag());
        self.ruler_markers(&ruler_response, timeline.sequence(), &scale, &mut output);
        if let Some(time) = self.scrub(&ruler_response, timeline.sequence(), &scale) {
            output.seek = Some(time);
        }

        let response = ui.interact(
            tracks,
            ui.scope_id().with("tracks"),
            Sense::click_and_drag(),
        );
        let top = tracks.top() - self.scroll_y;
        if !self.navigate(&response, tracks, timeline.sequence()) {
            self.handle_gestures(&response, timeline, scale, top, playhead, &mut output);
            self.show_cursor(&response, timeline.sequence(), &scale, top);
            self.show_drag_readout(&response, timeline.sequence());
            self.show_clip_tip(&response, timeline.sequence(), &scale, top);
        }
        output.clip_menu = self.context_clip.then(|| response.clone());
        output.gap_menu = self.context_gap.then(|| response.clone());
        let drop_hint = handle_drop(
            &response,
            timeline.sequence(),
            &self.heights,
            scale,
            top,
            self.snapping.then_some(playhead),
            &mut output,
        );

        let sequence = self
            .drag
            .as_ref()
            .and_then(|drag| drag.preview.as_ref())
            .unwrap_or(timeline.sequence());
        let rows = layout::rows(sequence, &self.heights, top);
        let painter = ui.painter_at(tracks);
        let edit_point = self
            .edit_point
            .as_ref()
            .filter(|point| point.is_on(sequence));
        let shown = painter.clip_rect().y_range();
        let states = paint::ClipStates::new(sequence, &self.selection);
        for row in rows.iter().filter(|row| row.y_range().intersects(shown)) {
            paint::row_background(
                &painter,
                tracks,
                row,
                sequence,
                self.targets.contains(row.track),
            );
            let automation = self.automation_shown.contains(&row.track);
            paint::clips(&painter, row, &scale, sequence, &states, art, !automation);
            let selected_gap = self.gap.filter(|&(track, _)| track == row.track);
            if let Some(range) = selected_gap.and_then(|(track, at)| gap(sequence, track, at)) {
                paint::gap(&painter, row, &scale, range);
            }
            if let Some(point) = edit_point {
                edit_point::paint(&painter, row, &scale, point);
            }
            if let Some(track) = sequence.track(row.track).filter(|_| automation) {
                let end = scale.time(tracks.right());
                let rect = layout::clip_rect(row, &scale, Ticks::ZERO, end).shrink(1.0);
                let envelope = rubber_band::Envelope::track(track, end);
                rubber_band::paint(&painter, rect, &scale, &envelope);
            }
        }
        match drop_hint {
            Some(hint) => drop::paint(&painter, hint, &rows, &scale, sequence, tracks),
            None if sequence.duration() == Ticks::ZERO => drop::paint_empty(&painter, tracks),
            None => {}
        }
        if let Some(at) = self.drag.as_ref().and_then(|drag| drag.snapped_to) {
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
                    &mut self.renaming,
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
                    report(
                        timeline.apply(SetTrackSwitch { track, switch, on }),
                        &mut output.errors,
                    );
                }
                HeaderAction::Rename(track, name) => {
                    report(
                        timeline.apply(RenameTrack {
                            track,
                            name: Some(name),
                        }),
                        &mut output.errors,
                    );
                }
                HeaderAction::Add(kind) => {
                    report(timeline.apply(AddTrack { kind }), &mut output.errors)
                }
                HeaderAction::Delete(track) => {
                    report(timeline.apply(RemoveTrack { track }), &mut output.errors)
                }
            }
        }
        self.track_scrollbar(ui, timeline.sequence(), tracks);
        self.time_scrollbar(ui, timeline.sequence(), time_bar);
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
            self.scroll_tracks(-scroll.y, sequence, tracks);
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

fn report(result: Result<(), timeline::TimelineError>, errors: &mut Vec<String>) {
    match result {
        Ok(()) | Err(timeline::TimelineError::NothingToEdit) => {}
        Err(error) => errors.push(error.to_string()),
    }
}

fn gap(sequence: &Sequence, track: TrackRef, at: Ticks) -> Option<std::ops::Range<Ticks>> {
    sequence.track(track)?.gap_at(at)
}
