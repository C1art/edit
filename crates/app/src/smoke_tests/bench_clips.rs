use std::path::Path;

use timeline::{Param, SetClipLook, SourceId, Ticks, TrackKind, VideoEffect};

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

/// Gives every video clip a Lumetri grade, touching each of its stages, and a Gaussian
/// blur, as a heavily finished cut has.
pub(super) fn add_effects(editor: &mut Editor) {
    let looks: Vec<_> = editor
        .timeline
        .sequence()
        .tracks_of(TrackKind::Video)
        .iter()
        .flat_map(|track| &track.clips)
        .map(|clip| {
            let mut look = clip.look.clone();
            let color = &mut look.color;
            color.temperature = Param::from_value(12.0);
            color.exposure = Param::from_value(0.3);
            color.contrast = Param::from_value(15.0);
            color.highlights = Param::from_value(-20.0);
            color.shadows = Param::from_value(10.0);
            color.midtone_wheel.red = Param::from_value(8.0);
            color.saturation = Param::from_value(110.0);
            color.vibrance = Param::from_value(20.0);
            color.faded_film = Param::from_value(15.0);
            color.sharpen = Param::from_value(30.0);
            color.vignette.amount = Param::from_value(-1.0);
            look.effects.push(VideoEffect::GaussianBlur {
                blurriness: Param::from_value(12.0),
            });
            SetClipLook {
                clip: clip.id,
                look,
            }
        })
        .collect();
    let count = looks.len();
    for look in looks {
        editor.apply(look);
    }
    eprintln!("Lumetri and Gaussian Blur on {count} clips");
}
