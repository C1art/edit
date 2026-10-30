//! Keeps one editor busy for minutes in real time, playing and editing by turns, and
//! checks that its threads, open files and memory level off rather than climb.
//!
//! On Linux, run it with `MALLOC_MMAP_THRESHOLD_=1048576`: glibc otherwise keeps the
//! frame buffers it frees in its heaps, and the resident size climbs with fragmentation
//! rather than with what the editor holds.

use std::time::{Duration, Instant};

use super::monkey::{crowded, frames, Monkey};
use super::{editor, gpu, Screen};
use crate::keymap::Command;
use crate::monitors::Focus;

/// Seconds of activity; `EDIT_SOAK_SECONDS` runs longer.
const SECONDS: u64 = 300;
const GESTURES_PER_ROUND: usize = 30;
/// Frames played per round, in real time.
const PLAYED_FRAMES: usize = 90;
const SAMPLE_EVERY: Duration = Duration::from_secs(10);
/// Resources held once the editor has warmed up may grow by this much at most. Media
/// the gestures add later fill caches the same way, such as the thumbnail readers,
/// each with a file and a decoder's threads.
const MORE_THREADS: usize = 32;
const MORE_FILES: usize = 8;
const MORE_MEGABYTES: u64 = 200;

#[derive(Debug, Clone, Copy)]
struct Usage {
    threads: usize,
    files: usize,
    megabytes: u64,
}

/// What the process holds, where the platform tells cheaply.
fn usage() -> Option<Usage> {
    let count = |dir| std::fs::read_dir(dir).ok().map(Iterator::count);
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let kilobytes: u64 = status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))?
        .trim()
        .trim_end_matches("kB")
        .trim()
        .parse()
        .ok()?;
    Some(Usage {
        threads: count("/proc/self/task")?,
        files: count("/proc/self/fd")?,
        megabytes: kilobytes / 1024,
    })
}

fn soak(gpu: Option<&eframe::egui_wgpu::RenderState>) {
    let seconds = std::env::var("EDIT_SOAK_SECONDS")
        .ok()
        .and_then(|seconds| seconds.parse().ok())
        .unwrap_or(SECONDS);
    let mut editor = editor(gpu);
    let mut screen = Screen::default();
    let mut monkey = Monkey::new(7);
    crowded(&mut editor);
    let start = Instant::now();
    let mut next_sample = start;
    let mut samples = Vec::new();
    while start.elapsed() < Duration::from_secs(seconds) {
        for _ in 0..GESTURES_PER_ROUND {
            for events in frames(&monkey.gesture()) {
                screen.draw(&mut editor, events);
            }
        }
        // The monkey may have undone or deleted everything there was to play.
        if !editor.timeline.sequence().has_clips() {
            crowded(&mut editor);
        }
        editor.focus = Focus::Program;
        crate::shortcuts::run(&mut editor, Command::GoToStart);
        crate::shortcuts::run(&mut editor, Command::PlayStop);
        for _ in 0..PLAYED_FRAMES {
            screen.draw(&mut editor, Vec::new());
            std::thread::sleep(Duration::from_millis(16));
        }
        if editor
            .program
            .as_ref()
            .is_some_and(|program| program.player.is_playing())
        {
            crate::shortcuts::run(&mut editor, Command::PlayStop);
        }
        if Instant::now() >= next_sample {
            next_sample += SAMPLE_EVERY;
            if let Some(usage) = usage() {
                // Gestures add and remove clips and media too, which caches follow.
                let project = editor.timeline.project();
                eprintln!(
                    "{:>5} s: {usage:?}, {} clips, {} media, {} undo steps",
                    start.elapsed().as_secs(),
                    project.all_clips().count(),
                    project.media.len(),
                    editor.timeline.done().count(),
                );
                samples.push(usage);
            }
        }
    }
    // The first fifth warms caches, worker pools and pipelines up.
    let (Some(warm), Some(last)) = (samples.get(samples.len() / 5), samples.last()) else {
        return;
    };
    assert!(
        last.threads <= warm.threads + MORE_THREADS
            && last.files <= warm.files + MORE_FILES
            && last.megabytes <= warm.megabytes + MORE_MEGABYTES,
        "grew from {warm:?} to {last:?}"
    );
}

#[test]
#[ignore = "takes minutes; decodes a real file with FFmpeg"]
fn a_long_session_levels_off() {
    soak(None);
}

#[test]
#[ignore = "takes minutes; needs a GPU, and decodes a real file with FFmpeg"]
fn a_long_session_levels_off_with_a_gpu() {
    soak(Some(&gpu()));
}
