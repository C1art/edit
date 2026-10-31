use std::path::{Path, PathBuf};

use egui::{Align2, Context, FontId, Id, LayerId, Order, Stroke, StrokeKind};

use crate::editor::Editor;
use crate::project_files::{is_project, is_subrip, is_xml};
use crate::theme;

/// Opens files dropped onto the window. While they're held over it, says what letting
/// go will do, as nothing else shows that the window takes them.
pub(crate) fn ui(editor: &mut Editor, ctx: &Context) {
    let (hovered, dropped) = ctx.input(|input| {
        let hovered: Vec<PathBuf> = input
            .raw
            .hovered_files
            .iter()
            .filter_map(|file| file.path.clone())
            .collect();
        let dropped: Vec<PathBuf> = input
            .raw
            .dropped_files
            .iter()
            .map(|file| file.path().to_owned())
            .filter(|path| !path.as_os_str().is_empty())
            .collect();
        (hovered, dropped)
    });
    for path in dropped {
        editor.open_path(path);
    }
    if !hovered.is_empty() {
        overlay(ctx, &what_dropping_does(&hovered));
    }
}

fn what_dropping_does(paths: &[PathBuf]) -> String {
    let name = |path: &Path| {
        path.file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
    };
    match paths {
        [path] if is_project(path) => format!("Open “{}”", name(path)),
        [path] if is_subrip(path) => format!("Import captions from “{}”", name(path)),
        [path] if is_xml(path) => format!("Import the sequence in “{}”", name(path)),
        [path] => format!("Import “{}”", name(path)),
        paths => format!("Import {} files", paths.len()),
    }
}

fn overlay(ctx: &Context, text: &str) {
    let painter = ctx.layer_painter(LayerId::new(Order::Foreground, Id::unique("file drop")));
    let screen = ctx.content_rect();
    painter.rect_filled(screen, 0.0, theme::GUTTER.gamma_multiply(0.75));
    painter.rect_stroke(
        screen.shrink(12.0),
        8.0,
        Stroke::new(2.0, theme::ACCENT),
        StrokeKind::Inside,
    );
    painter.text(
        screen.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(20.0),
        theme::TEXT,
    );
}
