//! Whether real footage plays smoothly through dissolves, and if not, what holds it up.
//! Cuts the files into short pieces, alternating between them, with the default
//! transition at every cut, then plays it all in real time: first only the Program
//! monitor's player and picture, then in the whole editor with sound. Run it in release:
//! `EDIT_BENCH_CLIPS="a.mp4;b.mp4" cargo test --release -p app playback_speed -- --ignored --nocapture`.

use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui_wgpu::{self, wgpu};
use playback::Player;
use timeline::{ApplyTransitions, PlaceClip, PlaceMode, Ticks, Timebase, TrackKind};

use super::{bench_clips, editor, gpu, Screen};
use crate::editor::Editor;
use crate::monitor::Monitor;
use crate::viewer::Viewer;

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
    /// Ticks that moved on by other than one frame: skipped or repeated, which judders
    /// even when nothing is dropped.
    uneven: u32,
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
             {} ticks uneven; each tick {:.1} ms on average, slowest {:.1} ms, {} over {:.1} ms",
            self.shown,
            self.dropped,
            self.behind,
            self.ticks,
            self.uneven,
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

    let mut pass = Pass::default();
    player.play();
    let mut next_tick = Instant::now();
    while player.is_playing() {
        wait_for(&mut next_tick);
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
        wait_on_gpu(&gpu);
        let tick = Tick {
            took: start.elapsed(),
            changed,
            due,
            dropped: player.dropped_frames() - dropped_before,
            building_shaders: monitor.building_shaders(),
        };
        pass.record(&player, tick);
    }
    drop((player, monitor));
    eprintln!("Playing alone, without sound or the rest of the editor:");
    pass.report();

    // Again in the whole editor, as when playing in the app: the panels drawn every
    // frame, and sound, whose clock the picture follows.
    let mut screen = Screen::default();
    editor.seek_program(Ticks::ZERO);
    let ready = Instant::now();
    while ready.elapsed() < Duration::from_secs(3) || editor_busy(&editor) {
        screen.draw(&mut editor, Vec::new());
        assert!(ready.elapsed() < Duration::from_secs(60), "never got ready");
        std::thread::sleep(TICK);
    }
    eprintln!(
        "In the editor, {}:",
        if editor.audio.is_some() {
            "with sound"
        } else {
            "without sound, as no output opened"
        }
    );
    let mut pass = Pass::default();
    program(&mut editor).player.play();
    let mut next_tick = Instant::now();
    while program(&mut editor).player.is_playing() {
        wait_for(&mut next_tick);
        let start = Instant::now();
        let dropped_before = program(&mut editor).player.dropped_frames();
        let shown_before = program(&mut editor)
            .player
            .current_frame()
            .map(|frame| frame.pts);
        screen.draw(&mut editor, Vec::new());
        let viewer = program(&mut editor);
        let due = rate.frame_at(viewer.player.position(), timebase);
        wait_on_gpu(&gpu);
        let tick = Tick {
            took: start.elapsed(),
            changed: viewer.player.current_frame().map(|frame| frame.pts) != shown_before,
            due,
            dropped: viewer.player.dropped_frames() - dropped_before,
            building_shaders: viewer.building_shaders(),
        };
        pass.record(&viewer.player, tick);
    }
    pass.report();
}

fn program(editor: &mut Editor) -> &mut Viewer {
    editor.program.as_mut().expect("a program monitor")
}

fn editor_busy(editor: &Editor) -> bool {
    editor
        .program
        .as_ref()
        .is_none_or(|viewer| viewer.player.current_frame().is_none() || viewer.building_shaders())
}

fn wait_for(next_tick: &mut Instant) {
    *next_tick += TICK;
    if let Some(wait) = next_tick.checked_duration_since(Instant::now()) {
        std::thread::sleep(wait);
    }
}

/// The app's window waits here for the GPU before showing the frame.
fn wait_on_gpu(gpu: &egui_wgpu::RenderState) {
    let _ = gpu.device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: None,
    });
}

struct Tick {
    took: Duration,
    changed: bool,
    /// The frame the clock had reached.
    due: i64,
    dropped: u64,
    building_shaders: bool,
}

#[derive(Default)]
struct Pass {
    transitions: Tally,
    elsewhere: Tally,
    waiting_on_shaders: u32,
    last_shown: Option<i64>,
}

impl Pass {
    fn record(&mut self, player: &Player, tick: Tick) {
        let Some(frame) = player.current_frame() else {
            return;
        };
        let sequence = player.sequence();
        let shown = player.frame_rate().frame_at(frame.pts, sequence.timebase);
        let tally = if frame.layers.len() > 1 {
            &mut self.transitions
        } else {
            &mut self.elsewhere
        };
        tally.ticks += 1;
        tally.shown += u32::from(tick.changed);
        tally.dropped += tick.dropped;
        tally.behind += u32::from(shown < tick.due);
        let moved = self.last_shown.map(|last| shown - last);
        tally.uneven += u32::from(moved.is_some_and(|moved| moved != 1));
        tally.over_budget += u32::from(tick.took > TICK);
        tally.total += tick.took;
        tally.slowest = tally.slowest.max(tick.took);
        self.waiting_on_shaders += u32::from(tick.building_shaders);
        self.last_shown = Some(shown);
    }

    fn report(&self) {
        eprintln!("  During dissolves: {}", self.transitions);
        eprintln!("  Elsewhere: {}", self.elsewhere);
        eprintln!(
            "  Ticks waiting on shaders being built: {}",
            self.waiting_on_shaders
        );
    }
}
