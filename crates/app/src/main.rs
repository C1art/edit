mod attributes_dialog;
mod autosave;
mod captions;
mod clip_art;
mod clip_menu;
mod commands;
mod editor;
mod effects;
mod export;
mod export_format;
mod gain_dialog;
mod history_panel;
mod import;
mod loudness_window;
mod marker_dialog;
mod markers_panel;
mod media_pool;
mod menus;
mod mixer;
mod monitor;
mod monitors;
mod multicam;
mod multicam_sync;
mod nesting;
mod panels;
mod placing;
mod project_files;
mod project_store;
mod proxies;
mod safe_margins;
mod scopes;
mod sequence_marks;
mod shortcuts;
mod source;
mod speed_dialog;
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
