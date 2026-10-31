//! The arrows skip in full screen rather than stepping a frame, and leaving puts the
//! speed back to normal.

use egui::{Event, Key, Modifiers};
use playback::PlayRate;
use timeline::Ticks;

use super::{editor, import, place, Screen, FRAME};

fn press(key: Key) -> Vec<Event> {
    [true, false]
        .map(|pressed| Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        })
        .into()
}

#[test]
fn arrows_skip_seconds_and_leaving_resets_the_speed() {
    let mut editor = editor(None);
    let mut screen = Screen::default();
    import(&mut editor, 25 * 20);
    place(&mut editor, 0, 25 * 20);
    editor.toggle_full_screen();
    screen.draw(&mut editor, Vec::new());
    let position = |editor: &crate::editor::Editor| {
        editor
            .program
            .as_ref()
            .map_or(Ticks::ZERO, |program| program.player.position())
    };
    screen.draw(&mut editor, press(Key::ArrowRight));
    screen.draw(&mut editor, press(Key::ArrowRight));
    screen.draw(&mut editor, press(Key::ArrowLeft));
    assert_eq!(position(&editor), Ticks(5 * 25 * FRAME));
    if let Some(program) = &mut editor.program {
        program.player.set_speed(PlayRate(150));
    }
    screen.draw(&mut editor, press(Key::Escape));
    assert!(!editor.full_screen);
    let speed = editor
        .program
        .as_ref()
        .map(|program| program.player.speed());
    assert_eq!(speed, Some(PlayRate::NORMAL));
}
