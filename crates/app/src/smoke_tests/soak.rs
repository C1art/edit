//! Keeps one editor busy for minutes in real time, playing and editing by turns, and
//! checks that its threads, handles and memory level off rather than climb.
//!
//! On Linux, run it with `MALLOC_MMAP_THRESHOLD_=1048576`: glibc otherwise keeps the
//! frame buffers it frees in its heaps, and the resident size climbs with fragmentation
//! rather than with what the editor holds.

use std::time::{Duration, Instant};

use super::monkey::{crowded, frames, Monkey};
use super::usage::usage;
use super::{editor, gpu, Screen};
use crate::clip_art::OPEN_READERS;
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
/// each with a file and a decoder's threads, but every cache is bounded, so none of
/// this grows with the number of media.
const MORE_THREADS: usize = 32;
/// Open files on Linux. Windows counts handles of every kind, which come and go with
/// the threads, about twenty a decoder thread.
const MORE_HANDLES: usize = if cfg!(windows) { 64 } else { 8 };
const HANDLES_PER_THREAD: usize = if cfg!(windows) { 24 } else { 0 };
const MORE_MEGABYTES: u64 = 200;
/// Each thumbnail reader left open keeps a decoder, with a frame pool per thread it
/// decodes on, which grows with the core count.
const MEGABYTES_PER_READER: u64 = 64;

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
    let (Some(&warm), Some(&last)) = (samples.get(samples.len() / 5), samples.last()) else {
        return;
    };
    let thread_handles = HANDLES_PER_THREAD * last.threads.saturating_sub(warm.threads);
    let reader_megabytes = MEGABYTES_PER_READER * OPEN_READERS as u64;
    assert!(
        last.threads <= warm.threads + MORE_THREADS
            && last.handles <= warm.handles + MORE_HANDLES + thread_handles
            && last.megabytes <= warm.megabytes + MORE_MEGABYTES + reader_megabytes,
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
