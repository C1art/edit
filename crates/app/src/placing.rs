use timeline::{
    edit_points, AddGenerated, AddTrack, ClipId, FourPoint, Generator, PlaceClip, PlaceMode,
    TrackKind,
};

use crate::editor::Editor;
use crate::timeline_view::{MediaDrop, Take};

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

/// Overwrites a media item dropped onto the timeline, keeping its video and audio
/// together on the hovered track and the first track of the other kind. An item from
/// the pool that the source monitor shows with marks brings only the marked part, as
/// in Premiere.
pub fn drop_media(editor: &mut Editor, drop: MediaDrop) {
    let sequence = editor.timeline.sequence();
    let Some(item) = sequence.media_item(drop.source) else {
        return;
    };
    let marked = editor
        .source
        .as_ref()
        .filter(|source| source.source == drop.source && source.marks.is_set())
        .map(|source| source.range());
    let (source_in, duration) = drop.range.or(marked).unwrap_or_else(|| {
        let range = item.default_range(sequence.timebase);
        (range.start, range.end - range.start)
    });
    let first = |count: usize| (count > 0).then_some(0);
    let (video_track, audio_track) = match drop.track.kind {
        TrackKind::Video => (Some(drop.track.index), first(sequence.audio.len())),
        TrackKind::Audio => (first(sequence.video.len()), Some(drop.track.index)),
    };
    let video_track = video_track.filter(|_| drop.take != Take::AudioOnly);
    let audio_track = audio_track.filter(|_| drop.take != Take::VideoOnly);
    editor.match_format(drop.source);
    editor.apply(PlaceClip {
        source: drop.source,
        source_in,
        duration,
        at: drop.at,
        video_track,
        audio_track,
        mode: PlaceMode::Overwrite,
        fill: None,
    });
}

/// Adds a title or color matte to the pool and lays it at the playhead on the first
/// free video track from the targeted one up, adding a track when all are busy, then
/// selects it for editing.
/// Adds `generator` to the project and places it at the playhead on the first free
/// video track, returning the placed clip.
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
