mod commands;
mod editor;
mod effects;
mod export;
mod import;
mod media_pool;
mod menus;
mod mixer;
mod monitor;
mod monitors;
mod placing;
mod proxies;
mod scopes;
mod shortcuts;
mod source;
mod timeline_view;
mod transport;
mod viewer;

use std::path::PathBuf;

use editor::Editor;

fn main() -> eframe::Result {
    let initial: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
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
