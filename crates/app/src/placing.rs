use timeline::{PlaceClip, PlaceMode, Ticks, TrackKind};

use crate::editor::Editor;
use crate::timeline_view::MediaDrop;

/// Edits the source monitor's marked range into the sequence at the program playhead,
/// on the targeted tracks, then parks the playhead after it.
pub fn edit_from_source(editor: &mut Editor, mode: PlaceMode) {
    let Some(source) = &editor.source else {
        editor.notice("Open a clip in the source monitor first");
        return;
    };
    let (source_in, duration) = source.range();
    let id = source.source;
    let targets = editor.view.targets;
    if targets.video.is_none() && targets.audio.is_none() {
        editor.notice("Target a video or audio track first (click a track name)");
        return;
    }
    let at = editor.playhead();
    editor.match_format(id);
    let placed = editor.apply(PlaceClip {
        source: id,
        source_in,
        duration,
        at,
        video_track: targets.video,
        audio_track: targets.audio,
        mode,
    });
    if placed {
        editor.seek_program(at + duration);
    }
}

/// Overwrites a whole media item dropped from the pool, keeping its video and audio
/// together on the hovered track and the first track of the other kind.
pub fn drop_media(editor: &mut Editor, drop: MediaDrop) {
    let sequence = editor.timeline.sequence();
    let Some(item) = sequence.media_item(drop.source) else {
        return;
    };
    let duration = item.duration;
    let first = |count: usize| (count > 0).then_some(0);
    let (video_track, audio_track) = match drop.track.kind {
        TrackKind::Video => (Some(drop.track.index), first(sequence.audio.len())),
        TrackKind::Audio => (first(sequence.video.len()), Some(drop.track.index)),
    };
    editor.match_format(drop.source);
    editor.apply(PlaceClip {
        source: drop.source,
        source_in: Ticks::ZERO,
        duration,
        at: drop.at,
        video_track,
        audio_track,
        mode: PlaceMode::Overwrite,
    });
}
