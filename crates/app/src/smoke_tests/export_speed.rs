//! How fast real footage exports with the Export Media dialog's default settings, and
//! where the time goes. Run it in release with the files to place one after another:
//! `EDIT_BENCH_CLIPS="a.mp4;b.mp4" cargo test --release -p app export_speed -- --ignored --nocapture`.
//! `EDIT_BENCH_SECONDS` uses at most that much of each file, `EDIT_BENCH_DISSOLVES=1`
//! puts the default transitions at every cut, and `EDIT_BENCH_SOFTWARE_DECODE=1` turns
//! hardware decoding off.

use std::time::Duration;

use playback::{ExportEvent, ExportJob};
use timeline::{ApplyTransitions, PlaceClip, PlaceMode, Ticks, Timebase, TrackKind};

use super::{editor, gpu};
use crate::export::{prepare, ExportDialog};

#[test]
#[ignore = "exports the files in EDIT_BENCH_CLIPS; meaningful only in release"]
fn export_speed() {
    let Ok(clips) = std::env::var("EDIT_BENCH_CLIPS") else {
        eprintln!("Set EDIT_BENCH_CLIPS to the files to export, separated by ';'");
        return;
    };
    let flag = |name| std::env::var(name).is_ok_and(|value| value == "1");
    let gpu = gpu();
    let mut editor = editor(Some(&gpu));
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
        let media = crate::import::probe(std::path::Path::new(path), timebase, rate)
            .unwrap_or_else(|error| panic!("{path}: {error}"));
        editor.apply(media);
    }
    let media: Vec<_> = (editor.timeline.sequence().media.iter())
        .map(|item| (item.id, item.duration))
        .collect();
    let longest = std::env::var("EDIT_BENCH_SECONDS")
        .ok()
        .and_then(|seconds| seconds.parse::<i64>().ok())
        .and_then(|seconds| Some(Ticks(seconds).rescale(Timebase::new(1, 1)?, timebase)));
    if let Some(&(first, _)) = media.first() {
        editor.match_format(first);
    }
    for (source, duration) in media {
        let at = editor.timeline.sequence().duration();
        editor.apply(PlaceClip {
            source,
            source_in: Ticks::ZERO,
            duration: longest.map_or(duration, |longest| duration.min(longest)),
            at,
            video_track: Some(0),
            audio_track: Some(0),
            mode: PlaceMode::Overwrite,
            fill: None,
        });
    }
    if flag("EDIT_BENCH_DISSOLVES") {
        let sequence = editor.timeline.sequence();
        let defaults = editor.transition_defaults;
        editor.apply(ApplyTransitions {
            clips: sequence.all_clips().map(|clip| clip.id).collect(),
            video: defaults.transition(sequence, TrackKind::Video),
            audio: defaults.transition(sequence, TrackKind::Audio),
        });
    }

    let mut dialog = ExportDialog::new(&editor);
    dialog.set_output(&std::env::temp_dir().join("edit-export-speed.mp4"));
    let (sequence, settings) = prepare(&mut editor, &dialog).expect("an exportable sequence");
    let format = sequence.format;
    eprintln!(
        "Exporting {}×{} at {:.2} fps to {}",
        format.width,
        format.height,
        format.frame_rate.approx_fps(),
        settings.path.display()
    );
    let job = ExportJob::start(sequence, gpu.device.clone(), gpu.queue.clone(), settings)
        .expect("an export");
    let mut shown_decoder = false;
    loop {
        if let Some(decoder) = media::hardware_decoder().filter(|_| !shown_decoder) {
            eprintln!("Decoding on {decoder}");
            shown_decoder = true;
        }
        for event in job.poll() {
            match event {
                ExportEvent::Encoder(encoder) => eprintln!("Encoder: {encoder}"),
                ExportEvent::Timings(timings) => eprintln!("{timings}"),
                ExportEvent::Warning(error) => eprintln!("Warning: {error}"),
                ExportEvent::Finished => return,
                ExportEvent::Failed(error) => panic!("Export failed: {error}"),
                ExportEvent::Cancelled => panic!("Export cancelled"),
                ExportEvent::Progress { .. } | ExportEvent::Measuring(_) => {}
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
