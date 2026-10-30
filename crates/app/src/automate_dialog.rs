//! Automate to Sequence: the items selected in the project laid into the sequence one
//! after another from the playhead, or one at each sequence marker.

use egui::{DragValue, Key};
use timeline::{
    AutomateToSequence, AutomatedPiece, MediaItem, PlaceMode, Sequence, SourceId, Ticks, TrackKind,
};

use crate::editor::Editor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ordering {
    Sort,
    Selection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Placement {
    InOrder,
    /// Each item at the next sequence marker, cut short where the marker after begins.
    AtMarkers,
}

pub(crate) struct AutomateDialog {
    ordering: Ordering,
    placement: Placement,
    mode: PlaceMode,
    /// Frames each still lasts, or `None` for each one's own length.
    still_frames: Option<i64>,
    video_transition: bool,
    audio_transition: bool,
    ignore_video: bool,
    ignore_audio: bool,
}

/// How many frames a still lasts when the dialog first sets one length for all.
const STILL_FRAMES: i64 = 150;

impl Editor {
    pub(crate) fn open_automate(&mut self) {
        if self.pool.selected.is_empty() {
            self.notice("Select items in the project first");
            return;
        }
        self.automate_dialog = Some(AutomateDialog {
            ordering: Ordering::Sort,
            placement: Placement::InOrder,
            mode: PlaceMode::Insert,
            still_frames: None,
            video_transition: true,
            audio_transition: true,
            ignore_video: false,
            ignore_audio: false,
        });
    }

    fn automate(&mut self, settings: &AutomateDialog) {
        let sequence = self.timeline.sequence();
        let sources = match settings.ordering {
            Ordering::Sort => self.pool.selected_in_sort_order(sequence),
            Ordering::Selection => self.pool.selected.clone(),
        };
        let takes = |item: &MediaItem| {
            (!settings.ignore_video && item.video.is_some())
                || (!settings.ignore_audio && item.has_audio)
        };
        let ranges: Vec<_> = sources
            .into_iter()
            .filter_map(|source| {
                let item = sequence.media_item(source).filter(|item| takes(item))?;
                Some((source, self.automated_range(sequence, item, settings)))
            })
            .collect();
        let Some(&(first, _)) = ranges.first() else {
            self.notice("None of the selected items has what's to be placed");
            return;
        };
        let targets = self.view.targets;
        let video_track = targets.video.filter(|_| !settings.ignore_video);
        let audio_track = targets.audio.filter(|_| !settings.ignore_audio);
        if video_track.is_none() && audio_track.is_none() {
            self.notice("Target a video or audio track first (click a track name)");
            return;
        }
        let start = self.playhead();
        let pieces = match settings.placement {
            Placement::InOrder => in_order(sequence, &ranges, start),
            Placement::AtMarkers => at_markers(sequence, &ranges, start),
        };
        let Some(end) = pieces.last().map(|piece| piece_end(sequence, piece)) else {
            self.notice("Add sequence markers after the playhead to place items at");
            return;
        };
        if pieces.len() < ranges.len() {
            let left = ranges.len() - pieces.len();
            self.notice(format!("{left} items had no marker left to go at"));
        }
        let defaults = self.transition_defaults;
        let sequence = self.timeline.sequence();
        let video_transition = settings
            .video_transition
            .then(|| defaults.transition(sequence, TrackKind::Video));
        let audio_transition = settings
            .audio_transition
            .then(|| defaults.transition(sequence, TrackKind::Audio));
        self.match_format(first);
        let mode = match settings.placement {
            Placement::InOrder => settings.mode,
            Placement::AtMarkers => PlaceMode::Overwrite,
        };
        if self.apply(AutomateToSequence {
            pieces,
            video_track,
            audio_track,
            mode,
            video_transition,
            audio_transition,
        }) {
            self.seek_program(end);
        }
    }

    /// The part of `item` to place: what's marked in the source monitor, or all of it,
    /// with a still lasting as the dialog says.
    fn automated_range(
        &self,
        sequence: &Sequence,
        item: &MediaItem,
        settings: &AutomateDialog,
    ) -> (Ticks, Ticks) {
        let marked = self.source_marked(item.id).unwrap_or_else(|| {
            let range = item.default_range(sequence.timebase);
            (range.start, range.end - range.start)
        });
        match settings.still_frames.filter(|_| item.still) {
            Some(frames) => {
                let rate = sequence.format.frame_rate;
                let length = rate.frame_start(frames, sequence.timebase);
                (Ticks::ZERO, length.min(item.duration))
            }
            None => marked,
        }
    }
}

fn in_order(
    sequence: &Sequence,
    ranges: &[(SourceId, (Ticks, Ticks))],
    start: Ticks,
) -> Vec<AutomatedPiece> {
    let mut at = start;
    ranges
        .iter()
        .map(|&(source, (source_in, duration))| {
            let piece = AutomatedPiece {
                source,
                source_in,
                duration,
                at,
            };
            at = piece_end(sequence, &piece);
            piece
        })
        .collect()
}

fn at_markers(
    sequence: &Sequence,
    ranges: &[(SourceId, (Ticks, Ticks))],
    start: Ticks,
) -> Vec<AutomatedPiece> {
    let mut marks: Vec<Ticks> = sequence
        .markers
        .iter()
        .map(|marker| marker.at)
        .filter(|&at| at >= start)
        .collect();
    marks.sort_unstable();
    marks.dedup();
    let next_marks = marks.iter().skip(1).map(Some).chain([None]);
    ranges
        .iter()
        .zip(marks.iter().zip(next_marks))
        .filter_map(|(&(source, (source_in, duration)), (&at, next))| {
            let conform = sequence.media_item(source)?.conform();
            let duration = match next {
                Some(&next) => duration.min(conform.source_span(next - at)),
                None => duration,
            };
            Some(AutomatedPiece {
                source,
                source_in,
                duration,
                at,
            })
        })
        .collect()
}

fn piece_end(sequence: &Sequence, piece: &AutomatedPiece) -> Ticks {
    let length = sequence
        .media_item(piece.source)
        .map_or(piece.duration, |item| {
            item.conform().sequence_span(piece.duration)
        });
    piece.at + length
}

pub(crate) fn dialog(editor: &mut Editor, ctx: &egui::Context) {
    let Some(settings) = &mut editor.automate_dialog else {
        return;
    };
    let mut done = false;
    let mut cancelled = false;
    crate::dialog::window(ctx, "Automate to Sequence").show(ctx, |ui| {
        egui::Grid::new("automate")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label("Ordering");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut settings.ordering, Ordering::Sort, "Sort Order");
                    let selection = Ordering::Selection;
                    ui.radio_value(&mut settings.ordering, selection, "Selection Order");
                });
                ui.end_row();
                ui.label("Placement");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut settings.placement, Placement::InOrder, "In Order");
                    ui.radio_value(
                        &mut settings.placement,
                        Placement::AtMarkers,
                        "At Sequence Markers",
                    )
                    .on_hover_text("Each item lasts until the next marker, as media allows");
                });
                ui.end_row();
                ui.label("Method");
                ui.add_enabled_ui(settings.placement == Placement::InOrder, |ui| {
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut settings.mode, PlaceMode::Insert, "Insert Edit");
                        let overwrite = PlaceMode::Overwrite;
                        ui.radio_value(&mut settings.mode, overwrite, "Overwrite Edit");
                    });
                })
                .response
                .on_disabled_hover_text("Items go over what's at the markers");
                ui.end_row();
                ui.label("Still clips");
                still_duration(ui, &mut settings.still_frames);
                ui.end_row();
                ui.label("Transitions");
                ui.vertical(|ui| {
                    let video = &mut settings.video_transition;
                    ui.checkbox(video, "Apply Default Video Transition");
                    let audio = &mut settings.audio_transition;
                    ui.checkbox(audio, "Apply Default Audio Transition");
                });
                ui.end_row();
                ui.label("Ignore");
                ui.horizontal(|ui| {
                    ui.checkbox(&mut settings.ignore_audio, "Audio");
                    ui.checkbox(&mut settings.ignore_video, "Video");
                });
                ui.end_row();
            });
        ui.weak("Items go from the playhead on the targeted tracks.");
        crate::dialog::footer(ui, |ui| {
            cancelled =
                ui.button("Cancel").clicked() || ui.input(|input| input.key_pressed(Key::Escape));
            done = ui.add(crate::dialog::primary("OK")).clicked()
                || ui.input(|input| input.key_pressed(Key::Enter));
        });
    });
    if !(done || cancelled) {
        return;
    }
    let Some(settings) = editor.automate_dialog.take() else {
        return;
    };
    if done {
        editor.automate(&settings);
    }
}

fn still_duration(ui: &mut egui::Ui, still_frames: &mut Option<i64>) {
    ui.vertical(|ui| {
        if ui
            .radio(still_frames.is_none(), "Use In/Out Range")
            .clicked()
        {
            *still_frames = None;
        }
        ui.horizontal(|ui| {
            let mut frames = still_frames.unwrap_or(STILL_FRAMES);
            if ui.radio(still_frames.is_some(), "").clicked() {
                *still_frames = Some(frames);
            }
            let field = DragValue::new(&mut frames).range(1..=100_000);
            if ui.add(field).changed() {
                *still_frames = Some(frames);
            }
            ui.label("frames per still");
        });
    });
}
