mod editor;
mod monitor;
mod transport;

use std::path::PathBuf;

use editor::Editor;

fn main() -> eframe::Result {
    let initial = std::env::args_os().nth(1).map(PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Edit")
            .with_inner_size([1600.0, 900.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native(
        "Edit",
        options,
        Box::new(|creation_context| Ok(Box::new(Editor::new(creation_context, initial)))),
    )
}
