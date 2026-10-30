//! How long the editor takes to draw a frame with a long, many-track sequence open,
//! while idle, zoomed out, selecting, scrolling and dragging. Run it in release:
//! `cargo test --release -p app frame_times -- --ignored --nocapture`.

use std::time::{Duration, Instant};

use egui::{Event, Modifiers, PointerButton, Pos2};
use timeline::{AddTrack, PlaceClip, PlaceMode, Ticks, TrackKind};

use super::{editor, import, Screen, FRAME};
use crate::keymap::Command;
use crate::monitors::Focus;

const TRACKS: usize = 10;
const CLIPS_PER_TRACK: i64 = 600;
const CLIP_FRAMES: i64 = 10;
const FRAMES: usize = 60;
/// A frame of 60 Hz, with room for what headless drawing leaves out.
const SLOWEST_MEAN: Duration = Duration::from_millis(16);

/// The mean time to draw each frame of `frames`.
fn timed(
    screen: &mut Screen,
    editor: &mut crate::editor::Editor,
    frames: Vec<Vec<Event>>,
) -> Duration {
    let count = frames.len().max(1) as u32;
    let start = Instant::now();
    for events in frames {
        screen.draw(editor, events);
    }
    start.elapsed() / count
}

fn idle() -> Vec<Vec<Event>> {
    vec![Vec::new(); FRAMES]
}

fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

#[test]
#[ignore = "measures frame times; meaningful only in release"]
fn frame_times_with_a_large_sequence() {
    let mut editor = editor(None);
    let mut screen = Screen::default();
    import(&mut editor, 25);
    for kind in [TrackKind::Video, TrackKind::Audio] {
        for _ in 1..TRACKS {
            editor.apply(AddTrack { kind });
        }
    }
    let source = editor.timeline.sequence().media[0].id;
    let start = Instant::now();
    for track in 0..TRACKS {
        for index in 0..CLIPS_PER_TRACK {
            editor.apply(PlaceClip {
                source,
                source_in: Ticks::ZERO,
                duration: Ticks(CLIP_FRAMES * FRAME),
                at: Ticks((index * CLIP_FRAMES + track as i64) * FRAME),
                video_track: Some(track),
                audio_track: Some(track),
                mode: PlaceMode::Overwrite,
                fill: None,
            });
        }
    }
    let placed = TRACKS as u32 * CLIPS_PER_TRACK as u32;
    eprintln!(
        "{} clips, {:?} a placing",
        editor.timeline.sequence().all_clips().count(),
        start.elapsed() / placed
    );
    // Warm up: fonts, layout caches, the first thumbnails asked for.
    timed(&mut screen, &mut editor, idle());

    let mut results = Vec::new();
    results.push(("idle", timed(&mut screen, &mut editor, idle())));
    editor.focus = Focus::Program;
    crate::shortcuts::run(&mut editor, Command::ZoomToFit);
    results.push(("zoomed to fit", timed(&mut screen, &mut editor, idle())));
    crate::shortcuts::run(&mut editor, Command::SelectAll);
    results.push(("all selected", timed(&mut screen, &mut editor, idle())));
    crate::shortcuts::run(&mut editor, Command::DeselectAll);
    for _ in 0..6 {
        crate::shortcuts::run(&mut editor, Command::ZoomIn);
    }
    let over_tracks = Pos2::new(800.0, 760.0);
    let scrolling = (0..FRAMES)
        .map(|_| {
            vec![
                Event::PointerMoved(over_tracks),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -40.0),
                    phase: egui::TouchPhase::Move,
                    source: egui::MouseWheelSource::Wheel,
                    modifiers: Modifiers::NONE,
                },
            ]
        })
        .collect();
    results.push(("scrolling", timed(&mut screen, &mut editor, scrolling)));
    let mut dragging = vec![
        vec![Event::PointerMoved(over_tracks)],
        vec![button(over_tracks, true)],
    ];
    dragging.extend((1..FRAMES).map(|step| {
        vec![Event::PointerMoved(
            over_tracks + egui::vec2(step as f32 * 3.0, 0.0),
        )]
    }));
    dragging.push(vec![button(over_tracks + egui::vec2(180.0, 0.0), false)]);
    results.push(("dragging", timed(&mut screen, &mut editor, dragging)));
    // What follows an edit, as the render bar's segments, is worked out on the next frame.
    let start = Instant::now();
    crate::shortcuts::run(&mut editor, Command::Undo);
    screen.draw(&mut editor, Vec::new());
    results.push(("undo and draw", start.elapsed()));

    for (what, mean) in &results {
        eprintln!("{what:>14}: {mean:?}");
    }
    let slow: Vec<_> = results
        .iter()
        .filter(|(_, mean)| *mean > SLOWEST_MEAN)
        .collect();
    assert!(slow.is_empty(), "slower than {SLOWEST_MEAN:?}: {slow:?}");
}
