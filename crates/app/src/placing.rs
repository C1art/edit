use std::ops::Range;

use timeline::{
    edit_points, AddGenerated, AddTrack, ClipId, FourPoint, Generator, PlaceClip, PlaceClips,
    PlaceMode, Sequence, SourceId, Ticks, Track, TrackKind,
};

use egui::DragAndDrop;

use crate::editor::Editor;
use crate::timeline_view::{drop_tracks, MediaDrop, SourceDrag, Take};

/// Edits the source monitor's marked range into the sequence on the targeted tracks,
/// placed by three-point editing, then parks the playhead after it. Marks on both
/// sides spanning different lengths ask how to fit the clip first.
pub fn edit_from_source(editor: &mut Editor, mode: PlaceMode) {
    place_from_source(editor, mode, None);
}

pub(crate) fn place_from_source(editor: &mut Editor, mode: PlaceMode, fit: Option<FourPoint>) {
    editor.as_one_edit(|editor| {
        let Some(source) = &editor.source else {
            editor.notice("Open a clip in the source monitor first");
            return;
        };
        let targets = editor.view.targets;
        if targets.video.is_none() && targets.audio.is_none() {
            editor.notice("Target a video or audio track first (click a track name)");
            return;
        }
        let sequence_marks = editor.timeline.sequence().marks;
        let points = edit_points(
            source.marks,
            source.duration(),
            sequence_marks,
            editor.playhead(),
            fit,
        );
        let points = match points {
            Ok(Some(points)) => points,
            Ok(None) => {
                editor.fit_clip = Some(mode);
                return;
            }
            Err(error) => {
                editor.notice(format!("Cannot edit: {error}"));
                return;
            }
        };
        let (source_in, duration) = source.media_range(points.source_in..points.source_out);
        let id = source.source;
        editor.match_format(id);
        let placed = editor.apply(PlaceClip {
            source: id,
            source_in,
            duration,
            at: points.at,
            video_track: targets.video,
            audio_track: targets.audio,
            mode,
            fill: points.fill,
        });
        if placed {
            let length = points.fill.unwrap_or(points.source_out - points.source_in);
            editor.seek_program(points.at + length);
        }
    })
}

/// The media being dragged. Dragging one of several items selected in the project
/// carries them all, in the order they were selected, and an item that was marked in
/// the source monitor carries only the marked part, as in Premiere.
pub fn carried_media(editor: &Editor, ctx: &egui::Context) -> Vec<SourceDrag> {
    if let Some(drag) = DragAndDrop::payload::<SourceDrag>(ctx) {
        return vec![*drag];
    }
    let Some(grabbed) = DragAndDrop::payload::<SourceId>(ctx).map(|id| *id) else {
        return Vec::new();
    };
    let selected = &editor.pool.selected;
    let sources = if selected.contains(&grabbed) {
        selected.as_slice()
    } else {
        std::slice::from_ref(&grabbed)
    };
    sources
        .iter()
        .filter_map(|&source| {
            let range = editor.source_marked(source).or_else(|| {
                let sequence = editor.timeline.sequence();
                let range = sequence
                    .media_item(source)?
                    .default_range(sequence.timebase);
                Some((range.start, range.end - range.start))
            })?;
            Some(SourceDrag {
                source,
                range,
                take: Take::Both,
            })
        })
        .collect()
}

/// Edits media dropped onto the timeline into it, keeping its video and audio together
/// on the hovered track and the first track of the other kind.
pub fn drop_media(editor: &mut Editor, drop: MediaDrop) {
    editor.as_one_edit(|editor| {
        let MediaDrop {
            media,
            track,
            at,
            mode,
        } = drop;
        let Some(first) = media.first() else {
            return;
        };
        editor.match_format(first.source);
        let (video_track, audio_track) = drop_tracks(editor.timeline.sequence(), &media, track);
        editor.apply(PlaceClips {
            pieces: pieces(&media),
            at,
            video_track,
            audio_track,
            mode,
        });
    })
}

/// Edits dragged media in at `at` on the targeted tracks, as a drop on the program
/// monitor does, then parks the playhead after it.
pub fn place_carried(editor: &mut Editor, media: &[SourceDrag], at: Ticks, mode: PlaceMode) {
    editor.as_one_edit(|editor| {
        let Some(first) = media.first() else {
            return;
        };
        let targets = editor.view.targets;
        let any_but = |take: Take| media.iter().any(|drag| drag.take != take);
        let video_track = targets.video.filter(|_| any_but(Take::AudioOnly));
        let audio_track = targets.audio.filter(|_| any_but(Take::VideoOnly));
        if video_track.is_none() && audio_track.is_none() {
            editor.notice("Target a video or audio track first (click a track name)");
            return;
        }
        editor.match_format(first.source);
        let length = carried_length(editor.timeline.sequence(), media);
        let placed = editor.apply(PlaceClips {
            pieces: pieces(media),
            at,
            video_track,
            audio_track,
            mode,
        });
        if placed {
            editor.seek_program(at + length);
        }
    })
}

/// Lays dragged media at the playhead on the first video track above the targeted one,
/// and the first audio track from the targeted one, that are free for its length,
/// adding tracks when none is, as Premiere's Overlay drop.
pub fn overlay_carried(editor: &mut Editor, media: &[SourceDrag]) {
    editor.as_one_edit(|editor| {
        let Some(first) = media.first() else {
            return;
        };
        editor.match_format(first.source);
        let sequence = editor.timeline.sequence();
        let at = editor.playhead();
        let span = at..at + carried_length(sequence, media);
        let has = |wanted: fn(&timeline::MediaItem, Take) -> bool| {
            media.iter().any(|drag| {
                sequence
                    .media_item(drag.source)
                    .is_some_and(|item| wanted(item, drag.take))
            })
        };
        let pictures = has(|item, take| item.video.is_some() && take != Take::AudioOnly);
        let sound = has(|item, take| item.has_audio && take != Take::VideoOnly);
        let targets = editor.view.targets;
        let above = targets.video.map_or(0, |index| index + 1);
        let video_track =
            pictures.then(|| free_or_new_track(editor, TrackKind::Video, above, &span));
        let from = targets.audio.unwrap_or(0);
        let audio_track = sound.then(|| free_or_new_track(editor, TrackKind::Audio, from, &span));
        let placed = editor.apply(PlaceClips {
            pieces: pieces(media),
            at,
            video_track,
            audio_track,
            mode: PlaceMode::Overwrite,
        });
        if placed {
            editor.seek_program(span.end);
        }
    })
}

/// How long dragged media lasts in the sequence, end to end.
fn carried_length(sequence: &Sequence, media: &[SourceDrag]) -> Ticks {
    media
        .iter()
        .filter_map(|drag| {
            let item = sequence.media_item(drag.source)?;
            Some(item.conform().sequence_span(drag.range.1))
        })
        .fold(Ticks::ZERO, |sum, span| sum + span)
}

/// The first unlocked track from `from` up with nothing over `span`.
fn free_track(tracks: &[Track], from: usize, span: &Range<Ticks>) -> Option<usize> {
    let free = |track: &Track| {
        !track.locked
            && track
                .clips
                .iter()
                .all(|clip| clip.end() <= span.start || clip.start >= span.end)
    };
    tracks
        .iter()
        .enumerate()
        .skip(from)
        .find(|(_, track)| free(track))
        .map(|(index, _)| index)
}

fn free_or_new_track(
    editor: &mut Editor,
    kind: TrackKind,
    from: usize,
    span: &Range<Ticks>,
) -> usize {
    let sequence = editor.timeline.sequence();
    let tracks = match kind {
        TrackKind::Video => &sequence.video,
        TrackKind::Audio => &sequence.audio,
    };
    if let Some(index) = free_track(tracks, from, span) {
        return index;
    }
    let index = tracks.len();
    editor.apply(AddTrack { kind });
    index
}

fn pieces(media: &[SourceDrag]) -> Vec<(SourceId, Ticks, Ticks)> {
    media
        .iter()
        .map(|drag| (drag.source, drag.range.0, drag.range.1))
        .collect()
}

/// Adds `generator` to the project and lays it at the playhead on the first free video
/// track from the targeted one up, adding a track when all are busy, then selects the
/// placed clip and returns it.
pub fn new_generated(editor: &mut Editor, generator: Generator) -> Option<ClipId> {
    let count = editor
        .timeline
        .sequence()
        .media
        .iter()
        .filter(|item| {
            item.generator
                .as_ref()
                .is_some_and(|other| other.kind_name() == generator.kind_name())
        })
        .count();
    let name = format!("{} {:02}", generator.kind_name(), count + 1);
    place_generated(editor, name, generator, None, Landing::Playhead)
}

/// Where a new clip goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Landing {
    /// At the playhead, on the first free video track from the targeted one up.
    Playhead,
    /// Where it was dropped, over what's there.
    Dropped { at: Ticks, video_track: usize },
}

/// As [`new_generated`], naming the media and placing `length` of it rather than the
/// usual still length.
pub fn place_generated(
    editor: &mut Editor,
    name: String,
    generator: Generator,
    length: Option<Ticks>,
    landing: Landing,
) -> Option<ClipId> {
    editor.as_one_edit(|editor| {
        let placed = editor.still_length.placed(editor.timeline.sequence());
        if !editor.apply(AddGenerated {
            name,
            generator,
            placed,
        }) {
            return None;
        }
        let sequence = editor.timeline.sequence();
        let item = sequence.media.last()?;
        let source = item.id;
        let range = item.default_range(sequence.timebase);
        let duration = length.map_or(range.end - range.start, |length| length.min(item.duration));
        let audio_track = item
            .has_audio
            .then(|| editor.view.targets.audio.unwrap_or(0))
            .filter(|&track| track < sequence.audio.len());
        let (at, track) = match landing {
            Landing::Playhead => {
                let at = editor.playhead();
                let first = editor.view.targets.video.unwrap_or(0);
                (
                    at,
                    free_or_new_track(editor, TrackKind::Video, first, &(at..at + duration)),
                )
            }
            Landing::Dropped { at, video_track } => (at, video_track),
        };
        let placed = editor.apply(PlaceClip {
            source,
            source_in: range.start,
            duration,
            at,
            video_track: Some(track),
            audio_track,
            mode: PlaceMode::Overwrite,
            fill: None,
        });
        let sequence = editor.timeline.sequence();
        let clip = sequence
            .video
            .get(track)
            .and_then(|track| track.clip_at(at))
            .filter(|_| placed)?
            .id;
        editor.view.selection = vec![clip];
        Some(clip)
    })
}
