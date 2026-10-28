mod editor;

use editor::Editor;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Edit")
            .with_inner_size([1600.0, 900.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Edit",
        options,
        Box::new(|_creation_context| Ok(Box::new(Editor::new()))),
    )
}
