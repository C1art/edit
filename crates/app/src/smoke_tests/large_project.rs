//! How a large project holds up: building it, saving it and opening it again through
//! the editor, then drawing frames while the timeline scrolls and zooms. Run it in
//! release: `cargo test --release -p app large_project -- --ignored --nocapture`.
//! `EDIT_BENCH_TRACKS` sets the tracks of each kind (20) and `EDIT_BENCH_TRACK_CLIPS`
//! the clips on each (1000).

use std::path::Path;
use std::time::{Duration, Instant};

use egui::{Event, Modifiers, Pos2};
use project::ProjectFile;
use timeline::{AddTrack, PlaceClip, PlaceMode, Ticks, TrackKind};

use super::{editor, import, Screen, FRAME, SCREEN};
use crate::editor::Editor;
use crate::keymap::Command;
use crate::project_files::AfterDiscard;

const CLIP_FRAMES: i64 = 10;
const GAP_FRAMES: i64 = 2;
const FRAMES: usize = 120;
const PATIENCE: Duration = Duration::from_secs(300);

fn setting(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1e3
}

#[test]
#[ignore = "builds, saves and opens a large project and times drawing it; meaningful only in release"]
fn large_project() {
    let tracks = setting("EDIT_BENCH_TRACKS", 20).max(1);
    let per_track = setting("EDIT_BENCH_TRACK_CLIPS", 1000) as i64;
    let mut editor = editor(None);
    let mut screen = Screen::default();
    import(&mut editor, 25);
    let source = editor.timeline.sequence().media[0].id;
    let started = Instant::now();
    for kind in [TrackKind::Video, TrackKind::Audio] {
        for _ in 1..tracks {
            editor.apply(AddTrack { kind });
        }
    }
    for track in 0..tracks {
        for index in 0..per_track {
            editor.apply(PlaceClip {
                source,
                source_in: Ticks::ZERO,
                duration: Ticks(CLIP_FRAMES * FRAME),
                at: Ticks((index * (CLIP_FRAMES + GAP_FRAMES) + track as i64) * FRAME),
                video_track: Some(track),
                audio_track: Some(track),
                mode: PlaceMode::Overwrite,
                fill: None,
            });
        }
    }
    let clips = editor.timeline.sequence().all_clips().count();
    eprintln!(
        "Built {tracks} video and {tracks} audio tracks, {clips} clips, in {:.0} ms",
        ms(started.elapsed())
    );

    let path = std::env::temp_dir().join("cledit-large-project.cledit");
    save(&mut editor, &mut screen, &path);
    let file = ProjectFile::new(editor.timeline.project().clone());
    let started = Instant::now();
    let text = file.to_text().expect("the project as text");
    let to_text = started.elapsed();
    let started = Instant::now();
    ProjectFile::from_text(&text).expect("the project read back");
    eprintln!(
        "  of which writing the text {:.0} ms, and reading it back {:.0} ms",
        ms(to_text),
        ms(started.elapsed())
    );

    let mut opened = editor_with(&path, clips);
    frame_times(&mut opened);
    let _ = std::fs::remove_file(&path);
}

fn save(editor: &mut Editor, screen: &mut Screen, path: &Path) {
    let started = Instant::now();
    editor.save_project_to(path.to_owned());
    while editor.saved_revision != editor.timeline.revision() {
        assert!(started.elapsed() < PATIENCE, "never saved");
        screen.draw(editor, Vec::new());
        std::thread::sleep(Duration::from_millis(1));
    }
    let bytes = std::fs::metadata(path).map_or(0, |meta| meta.len());
    eprintln!(
        "Saved in {:.0} ms through the editor: {:.1} MB",
        ms(started.elapsed()),
        bytes as f64 / 1e6
    );
}

/// A new editor with the project at `path` open and drawn once.
fn editor_with(path: &Path, clips: usize) -> Editor {
    let mut editor = editor(None);
    let mut screen = Screen::default();
    screen.draw(&mut editor, Vec::new());
    let started = Instant::now();
    editor.after_discard(AfterDiscard::Open(path.to_owned()));
    while editor.timeline.sequence().all_clips().count() != clips {
        assert!(started.elapsed() < PATIENCE, "never opened");
        screen.draw(&mut editor, Vec::new());
        std::thread::sleep(Duration::from_millis(1));
    }
    let read = started.elapsed();
    screen.draw(&mut editor, Vec::new());
    eprintln!(
        "Opened in {:.0} ms through the editor, first frame drawn after {:.0} ms",
        ms(read),
        ms(started.elapsed())
    );
    editor
}

/// Each frame's time to lay out the editor and to tessellate what it drew.
struct Times(Vec<(Duration, Duration)>);

impl Times {
    fn report(&self, what: &str) {
        let mut totals: Vec<_> = self.0.iter().map(|(ui, shapes)| *ui + *shapes).collect();
        totals.sort();
        let count = totals.len().max(1) as u32;
        let mean = totals.iter().sum::<Duration>() / count;
        let tessellating = self.0.iter().map(|(_, shapes)| *shapes).sum::<Duration>() / count;
        let p95 = totals
            .get(totals.len() * 95 / 100)
            .copied()
            .unwrap_or_default();
        let slowest = totals.last().copied().unwrap_or_default();
        eprintln!(
            "{what:>22}: mean {:5.2} ms (tessellating {:4.2}), 95th percentile {:5.2} ms, \
             slowest {:5.2} ms",
            ms(mean),
            ms(tessellating),
            ms(p95),
            ms(slowest)
        );
    }
}

fn timed(screen: &mut Screen, editor: &mut Editor, frames: Vec<Vec<Event>>) -> Times {
    Times(
        frames
            .into_iter()
            .map(|events| {
                let started = Instant::now();
                let mut output = screen.run(editor, events);
                output.textures_delta.clear();
                let laid_out = started.elapsed();
                let started = Instant::now();
                std::hint::black_box(
                    editor
                        .ctx
                        .tessellate(output.shapes, output.pixels_per_point),
                );
                (laid_out, started.elapsed())
            })
            .collect(),
    )
}

fn wheel(at: Pos2, delta: egui::Vec2) -> Vec<Event> {
    vec![
        Event::PointerMoved(at),
        Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta,
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Wheel,
            modifiers: Modifiers::NONE,
        },
    ]
}

/// A point over the timeline's clips, found by scrolling across at each height until the
/// timeline moves.
fn over_clips(screen: &mut Screen, editor: &mut Editor) -> Pos2 {
    let x = SCREEN.x * 0.6;
    (0..)
        .map(|step| SCREEN.y - 20.0 - step as f32 * 20.0)
        .take_while(|&y| y > 0.0)
        .map(|y| Pos2::new(x, y))
        .find(|&at| {
            let before = editor.view.scrolled();
            screen.draw(editor, wheel(at, egui::vec2(-80.0, 0.0)));
            screen.draw(editor, Vec::new());
            editor.view.scrolled().0 != before.0
        })
        .expect("the timeline somewhere on screen")
}

fn frame_times(editor: &mut Editor) {
    let mut screen = Screen::default();
    // Warm up: fonts, layout caches, the first thumbnails asked for.
    timed(&mut screen, editor, vec![Vec::new(); FRAMES]);
    let at = over_clips(&mut screen, editor);
    eprintln!("Frame times over {FRAMES} frames, timeline at {at:?}:");
    timed(&mut screen, editor, vec![Vec::new(); FRAMES]).report("idle");
    let down = (0..FRAMES)
        .map(|_| wheel(at, egui::vec2(0.0, -40.0)))
        .collect();
    timed(&mut screen, editor, down).report("scrolling down tracks");
    let across = (0..FRAMES)
        .map(|_| wheel(at, egui::vec2(-60.0, 0.0)))
        .collect();
    timed(&mut screen, editor, across).report("scrolling across");
    let zoom = |factor: f32| {
        (0..FRAMES)
            .map(|_| vec![Event::PointerMoved(at), Event::Zoom(factor)])
            .collect()
    };
    timed(&mut screen, editor, zoom(0.95)).report("zooming out");
    timed(&mut screen, editor, zoom(1.0 / 0.95)).report("zooming in");
    crate::shortcuts::run(editor, Command::ZoomToFit);
    timed(&mut screen, editor, vec![Vec::new(); FRAMES]).report("zoomed to fit");
    let across = (0..FRAMES)
        .map(|_| wheel(at, egui::vec2(0.0, -40.0)))
        .collect();
    timed(&mut screen, editor, across).report("fit, scrolling tracks");
}
