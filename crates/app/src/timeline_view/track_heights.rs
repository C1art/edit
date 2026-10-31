use std::collections::BTreeMap;

use egui::{CursorIcon, Rect, Sense};
use timeline::{Sequence, TrackKind, TrackRef};

use super::layout::Row;
use crate::track_follow::TrackMoves;

pub const DEFAULT_HEIGHT: f32 = 46.0;
/// Just fits the header's switches.
const MIN_HEIGHT: f32 = 28.0;
const MAX_HEIGHT: f32 = 240.0;
/// What Expand All Tracks sets every track to.
const EXPANDED_HEIGHT: f32 = 120.0;
/// Each Increase or Decrease Track Heights step scales by this.
const STEP: f32 = 1.25;
/// How far either side of a track's lower edge grabs it, in points.
const GRIP: f32 = 3.0;

/// How tall each track is drawn; tracks never resized have the default height.
#[derive(Debug, Clone, Default)]
pub struct TrackHeights(BTreeMap<TrackRef, f32>);

impl TrackHeights {
    pub fn of(&self, track: TrackRef) -> f32 {
        self.0.get(&track).copied().unwrap_or(DEFAULT_HEIGHT)
    }

    pub fn set(&mut self, track: TrackRef, height: f32) {
        self.0.insert(track, height.clamp(MIN_HEIGHT, MAX_HEIGHT));
    }

    pub fn follow_tracks(&mut self, moves: &TrackMoves) {
        let heights = std::mem::take(&mut self.0).into_iter();
        self.0 = heights
            .filter_map(|(track, height)| Some((moves.of(track)?, height)))
            .collect();
    }

    /// Steps every track of `kind` taller, or shorter when `taller` is false.
    pub fn step(&mut self, sequence: &Sequence, kind: TrackKind, taller: bool) {
        let factor = if taller { STEP } else { 1.0 / STEP };
        for track in tracks(sequence).filter(|track| track.kind == kind) {
            self.set(track, (self.of(track) * factor).round());
        }
    }

    /// Makes every track tall enough to read its clips, or as short as it goes.
    pub fn all(&mut self, sequence: &Sequence, expanded: bool) {
        let height = if expanded {
            EXPANDED_HEIGHT
        } else {
            MIN_HEIGHT
        };
        for track in tracks(sequence) {
            self.set(track, height);
        }
    }
}

fn tracks(sequence: &Sequence) -> impl Iterator<Item = TrackRef> {
    let video = (0..sequence.video.len()).map(TrackRef::video);
    video.chain((0..sequence.audio.len()).map(TrackRef::audio))
}

/// Lets each track's lower edge in the header column be dragged to resize it.
pub fn resize_grips(heights: &mut TrackHeights, ui: &egui::Ui, rows: &[Row], column: Rect) {
    for row in rows {
        let grip = Rect::from_x_y_ranges(
            column.x_range(),
            (row.bottom() - GRIP)..=(row.bottom() + GRIP),
        )
        .intersect(column);
        let id = ui.scope_id().with(("track_height", row.track));
        let response = ui
            .interact(grip, id, Sense::drag())
            .on_hover_cursor(CursorIcon::ResizeVertical);
        if response.dragged() {
            heights.set(row.track, row.height + response.drag_delta().y);
        }
    }
}
