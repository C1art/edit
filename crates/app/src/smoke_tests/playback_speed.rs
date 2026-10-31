//! Whether real footage plays smoothly through dissolves, and if not, what holds it up.
//! Cuts the files into short pieces, alternating between them, with the default
//! transition at every cut, then plays it all in real time as the Program monitor does,
//! without sound. Run it in release:
//! `EDIT_BENCH_CLIPS="a.mp4;b.mp4" cargo test --release -p app playback_speed -- --ignored --nocapture`.

use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui_wgpu::wgpu;
use playback::Player;
use timeline::{ApplyTransitions, PlaceClip, PlaceMode, Ticks, Timebase, TrackKind};

use super::{bench_clips, editor, gpu};
use crate::monitor::Monitor;

const PIECES: i64 = 8;
const PIECE_SECONDS: i64 = 4;
/// Where the first piece of each file starts, and how far apart pieces of one file are,
/// so most start between keyframes as real edits do.
const FIRST_IN_SECONDS: i64 = 5;
const STRIDE_SECONDS: i64 = 7;
/// A 60 Hz display's refresh.
const TICK: Duration = Duration::from_nanos(16_666_667);

/// What happened over the ticks spent on one kind of frame.
#[derive(Default)]
struct Tally {
    ticks: u32,
    shown: u32,
    dropped: u64,
    /// Ticks still showing an older frame than the one due, as it wasn't decoded yet.
    behind: u32,
    over_budget: u32,
    total: Duration,
    slowest: Duration,
}

impl fmt::Display for Tally {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mean = self.total / self.ticks.max(1);
        write!(
            f,
            "{} frames shown, {} dropped; behind on decoding at {} of {} ticks; \
             each tick {:.1} ms on average, slowest {:.1} ms, {} over {:.1} ms",
            self.shown,
            self.dropped,
            self.behind,
            self.ticks,
            mean.as_secs_f64() * 1e3,
            self.slowest.as_secs_f64() * 1e3,
            self.over_budget,
            TICK.as_secs_f64() * 1e3,
        )
    }
}

#[test]
#[ignore = "plays the files in EDIT_BENCH_CLIPS in real time; meaningful only in release"]
fn playback_speed() {
    let gpu = gpu();
    let adapter = gpu.adapter.get_info();
    eprintln!("Drawing on {} with {:?}", adapter.name, adapter.backend);
    let mut editor = editor(Some(&gpu));
    let Some(media) = bench_clips::import(&mut editor) else {
        return;
    };
    let timebase = editor.timeline.sequence().timebase;
    let seconds = |seconds: i64| {
        Timebase::new(1, 1).map_or(Ticks::ZERO, |one| Ticks(seconds).rescale(one, timebase))
    };
    for piece in 0..PIECES {
        let Some(&(source, duration)) = media.get(piece as usize % media.len().max(1)) else {
            return;
        };
        let round = piece / media.len() as i64;
        let room = (duration - seconds(PIECE_SECONDS)).max(Ticks(1));
        let source_in = Ticks((seconds(FIRST_IN_SECONDS + round * STRIDE_SECONDS)).0 % room.0);
        let at = editor.timeline.sequence().duration();
        editor.apply(PlaceClip {
            source,
            source_in,
            duration: seconds(PIECE_SECONDS).min(duration),
            at,
            video_track: Some(0),
            audio_track: Some(0),
            mode: PlaceMode::Overwrite,
            fill: None,
        });
    }
    let sequence = editor.timeline.sequence();
    let defaults = editor.transition_defaults;
    editor.apply(ApplyTransitions {
        clips: sequence.all_clips().map(|clip| clip.id).collect(),
        video: defaults.transition(sequence, TrackKind::Video),
        audio: defaults.transition(sequence, TrackKind::Audio),
    });

    let sequence = Arc::new(editor.timeline.sequence().clone());
    let format = sequence.format;
    let (rate, timebase) = (format.frame_rate, sequence.timebase);
    eprintln!(
        "Playing {} pieces of {PIECE_SECONDS} s with dissolves, {}×{} at {:.2} fps",
        PIECES,
        format.width,
        format.height,
        rate.approx_fps()
    );
    let ctx = egui::Context::default();
    let mut player = Player::new(Arc::clone(&sequence), None).expect("a player");
    let mut monitor = Monitor::new(gpu.clone());
    let canvas = (format.width, format.height);
    // The app builds its shaders at start, long before anything plays.
    let ready = Instant::now();
    while player.current_frame().is_none() || monitor.building_shaders() {
        if player.update() {
            if let Some(frame) = player.current_frame() {
                monitor.present(frame, canvas).expect("a frame shown");
            }
        }
        monitor.catch_up(&ctx).expect("a frame shown");
        assert!(ready.elapsed() < Duration::from_secs(60), "never got ready");
        std::thread::sleep(Duration::from_millis(5));
    }
    if let Some(decoder) = media::hardware_decoder() {
        eprintln!("Decoding on {decoder}");
    }

    let (mut transitions, mut elsewhere) = (Tally::default(), Tally::default());
    let mut waiting_on_shaders = 0;
    player.play();
    let mut next_tick = Instant::now();
    while player.is_playing() {
        next_tick += TICK;
        if let Some(wait) = next_tick.checked_duration_since(Instant::now()) {
            std::thread::sleep(wait);
        }
        let start = Instant::now();
        let dropped_before = player.dropped_frames();
        let changed = player.update();
        // Taken before drawing, which a slow tick would otherwise count as decoding.
        let due = rate.frame_at(player.position(), timebase);
        if changed {
            if let Some(frame) = player.current_frame() {
                monitor.present(frame, canvas).expect("a frame shown");
            }
        }
        monitor.catch_up(&ctx).expect("a frame shown");
        // The app's window waits here for the GPU before showing the frame.
        let _ = gpu.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });
        let took = start.elapsed();
        let Some(frame) = player.current_frame() else {
            continue;
        };
        let tally = if frame.layers.len() > 1 {
            &mut transitions
        } else {
            &mut elsewhere
        };
        tally.ticks += 1;
        tally.shown += u32::from(changed);
        tally.dropped += player.dropped_frames() - dropped_before;
        tally.behind += u32::from(rate.frame_at(frame.pts, timebase) < due);
        tally.over_budget += u32::from(took > TICK);
        tally.total += took;
        tally.slowest = tally.slowest.max(took);
        waiting_on_shaders += u32::from(monitor.building_shaders());
    }
    eprintln!("During dissolves: {transitions}");
    eprintln!("Elsewhere: {elsewhere}");
    eprintln!("Ticks waiting on shaders being built: {waiting_on_shaders}");
}
