use std::path::Path;

use timeline::{SourceId, Ticks};

use crate::editor::Editor;

pub(super) fn flag(name: &str) -> bool {
    std::env::var(name).is_ok_and(|value| value == "1")
}

/// Imports the files in `EDIT_BENCH_CLIPS`, separated by ';', returning each one's
/// source and duration, or None when the variable isn't set.
pub(super) fn import(editor: &mut Editor) -> Option<Vec<(SourceId, Ticks)>> {
    let Ok(clips) = std::env::var("EDIT_BENCH_CLIPS") else {
        eprintln!("Set EDIT_BENCH_CLIPS to the files to use, separated by ';'");
        return None;
    };
    if flag("EDIT_BENCH_SOFTWARE_DECODE") {
        media::set_hardware_decoding(false);
    }
    let (timebase, rate) = {
        let sequence = editor.timeline.sequence();
        (sequence.timebase, sequence.format.frame_rate)
    };
    for path in clips
        .split(';')
        .map(str::trim)
        .filter(|path| !path.is_empty())
    {
        let media = crate::import::probe(Path::new(path), timebase, rate)
            .unwrap_or_else(|error| panic!("{path}: {error}"));
        editor.apply(media);
    }
    let media: Vec<_> = (editor.timeline.sequence().media.iter())
        .map(|item| (item.id, item.duration))
        .collect();
    if let Some(&(first, _)) = media.first() {
        editor.match_format(first);
    }
    Some(media)
}
