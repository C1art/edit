//! How fast real footage exports with the Export Media dialog's default settings, and
//! where the time goes. Run it in release with the files to place one after another:
//! `EDIT_BENCH_CLIPS="a.mp4;b.mp4" cargo test --release -p app export_speed -- --ignored --nocapture`.
//! `EDIT_BENCH_SECONDS` uses at most that much of each file, `EDIT_BENCH_DISSOLVES=1`
//! puts the default transitions at every cut, `EDIT_BENCH_EFFECTS=1` grades and blurs
//! every clip, and `EDIT_BENCH_SOFTWARE_DECODE=1` turns hardware decoding off.

use std::time::Duration;

use playback::{ExportEvent, ExportJob};
use timeline::{ApplyTransitions, PlaceClip, PlaceMode, Ticks, Timebase, TrackKind};

use super::{bench_clips, editor, gpu};
use crate::export::{prepare, ExportDialog};

#[test]
#[ignore = "exports the files in EDIT_BENCH_CLIPS; meaningful only in release"]
fn export_speed() {
    let gpu = gpu();
    let adapter = gpu.adapter.get_info();
    eprintln!("Drawing on {} with {:?}", adapter.name, adapter.backend);
    let mut editor = editor(Some(&gpu));
    let Some(media) = bench_clips::import(&mut editor) else {
        return;
    };
    let timebase = editor.timeline.sequence().timebase;
    let longest = std::env::var("EDIT_BENCH_SECONDS")
        .ok()
        .and_then(|seconds| seconds.parse::<i64>().ok())
        .and_then(|seconds| Some(Ticks(seconds).rescale(Timebase::new(1, 1)?, timebase)));
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
    if bench_clips::flag("EDIT_BENCH_DISSOLVES") {
        let sequence = editor.timeline.sequence();
        let defaults = editor.transition_defaults;
        editor.apply(ApplyTransitions {
            clips: sequence.all_clips().map(|clip| clip.id).collect(),
            video: defaults.transition(sequence, TrackKind::Video),
            audio: defaults.transition(sequence, TrackKind::Audio),
        });
    }
    if bench_clips::flag("EDIT_BENCH_EFFECTS") {
        bench_clips::add_effects(&mut editor);
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
