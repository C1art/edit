use super::{editor, Screen, SCREEN};
use crate::editor::Editor;

fn draw(editor: &mut Editor, screen: &mut Screen, size: egui::Vec2) {
    screen.size = size;
    for _ in 0..3 {
        screen.draw(editor, Vec::new());
    }
}

#[test]
fn minimizing_the_window_leaves_the_layout_as_it_was() {
    let mut editor = editor(None);
    let mut screen = Screen::default();
    draw(&mut editor, &mut screen, SCREEN);
    let before = editor.workspaces.current();
    // What Windows reports for a minimized window.
    draw(&mut editor, &mut screen, egui::vec2(160.0, 28.0));
    draw(&mut editor, &mut screen, SCREEN);
    assert_eq!(editor.workspaces.current(), before);
}
