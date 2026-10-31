//! Runs every command, on the program and on the source, against timelines that are
//! empty, tiny or crowded, drawing the whole window after each, and reports any that
//! panic. A command that has nothing to act on should do nothing, not take the editor
//! down.

use std::panic::{catch_unwind, AssertUnwindSafe};

use eframe::egui_wgpu;
use timeline::{Marks, SetMarks, Ticks};

use super::{editor, gpu, import, message, place, Screen, FRAME};
use crate::editor::Editor;
use crate::keymap::Command;
use crate::monitors::Focus;

type Setup = fn(&mut Editor);

const SETUPS: [(&str, Setup); 6] = [
    ("empty", |_| {}),
    ("media but no clips", |editor| import(editor, 25)),
    ("one frame clip", |editor| {
        import(editor, 25);
        place(editor, 0, 1);
    }),
    ("clips with a gap, all selected", |editor| {
        import(editor, 25);
        place(editor, 0, 10);
        place(editor, 20, 5);
        crate::shortcuts::run(editor, Command::SelectAll);
    }),
    ("marks in equal out", |editor| {
        import(editor, 25);
        place(editor, 0, 10);
        editor.apply(SetMarks {
            marks: Marks {
                mark_in: Some(Ticks(5 * FRAME)),
                mark_out: Some(Ticks(5 * FRAME)),
            },
        });
    }),
    ("source open", |editor| {
        import(editor, 25);
        if let Some(media) = editor.timeline.sequence().media.first().map(|m| m.id) {
            editor.open_source(media);
        }
        editor.focus = Focus::Source;
    }),
];

fn run_every_command(gpu: Option<&egui_wgpu::RenderState>) {
    let mut panics = Vec::new();
    for (name, setup) in SETUPS {
        let mut editor = editor(gpu);
        let mut screen = Screen::default();
        setup(&mut editor);
        screen.draw(&mut editor, Vec::new());
        for command in Command::ALL {
            let ran = catch_unwind(AssertUnwindSafe(|| {
                crate::shortcuts::run(&mut editor, command);
                screen.draw(&mut editor, Vec::new());
                screen.draw(&mut editor, Vec::new());
            }));
            if let Err(payload) = ran {
                panics.push(format!("{name}: {command:?}: {}", message(&*payload)));
                editor = self::editor(gpu);
                screen = Screen::default();
                setup(&mut editor);
            }
        }
    }
    assert!(panics.is_empty(), "\n{}", panics.join("\n"));
}

#[test]
fn no_command_panics_on_a_degenerate_timeline() {
    run_every_command(None);
}

#[test]
#[ignore = "needs a GPU, and decodes a real file with FFmpeg"]
fn no_command_panics_on_a_degenerate_timeline_with_a_gpu() {
    run_every_command(Some(&gpu()));
}
