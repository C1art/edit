use timeline::{
    edit_points, AddGenerated, AddTrack, ClipId, FourPoint, Generator, PlaceClip, PlaceClips,
    PlaceMode, SourceId, TrackKind,
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
    let pieces = media
        .iter()
        .map(|drag| (drag.source, drag.range.0, drag.range.1))
        .collect();
    editor.apply(PlaceClips {
        pieces,
        at,
        video_track,
        audio_track,
        mode,
    });
}

/// Adds `generator` to the project and lays it at the playhead on the first free video
/// track from the targeted one up, adding a track when all are busy, then selects the
/// placed clip and returns it.
pub fn new_generated(editor: &mut Editor, generator: Generator) -> Option<ClipId> {
    let sequence = editor.timeline.sequence();
    let count = sequence
        .media
        .iter()
        .filter(|item| {
            item.generator
                .as_ref()
                .is_some_and(|other| other.kind_name() == generator.kind_name())
        })
        .count();
    let name = format!("{} {:02}", generator.kind_name(), count + 1);
    if !editor.apply(AddGenerated { name, generator }) {
        return None;
    }
    let sequence = editor.timeline.sequence();
    let item = sequence.media.last()?;
    let source = item.id;
    let range = item.default_range(sequence.timebase);
    let duration = range.end - range.start;
    let audio_track = item
        .has_audio
        .then(|| editor.view.targets.audio.unwrap_or(0))
        .filter(|&track| track < sequence.audio.len());
    let at = editor.playhead();
    let first = editor.view.targets.video.unwrap_or(0);
    let free = sequence
        .video
        .iter()
        .enumerate()
        .skip(first)
        .find(|(_, track)| {
            !track.locked
                && track
                    .clips
                    .iter()
                    .all(|clip| clip.end() <= at || clip.start >= at + duration)
        })
        .map(|(index, _)| index);
    let track = match free {
        Some(index) => index,
        None => {
            let index = sequence.video.len();
            editor.apply(AddTrack {
                kind: TrackKind::Video,
            });
            index
        }
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
}
