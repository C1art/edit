mod attributes_dialog;
mod autosave;
mod captions;
mod channels_dialog;
mod clip_art;
mod clip_menu;
mod clip_moves;
mod color_match;
mod commands;
mod comparison;
mod duck_dialog;
mod editor;
mod effect_presets;
mod effects;
mod effects_panel;
mod essential_sound;
mod export;
mod export_format;
mod export_frame;
mod export_queue;
mod find_dialog;
mod fit_clip;
mod gain_dialog;
mod graphics_tools;
mod history_panel;
mod icons;
mod import;
mod installed_fonts;
mod interpret_dialog;
mod keyboard_trim;
mod keymap;
mod link_media;
mod loudness_window;
mod marker_dialog;
mod markers_panel;
mod mask_tracking;
mod media_pool;
mod menus;
mod metadata_panel;
mod mixer;
mod monitor;
mod monitors;
mod motion_analysis;
mod multicam;
mod multicam_sync;
mod nesting;
mod offline;
mod panels;
mod path_dialog;
mod placing;
mod program_zoom;
mod project_files;
mod project_manager;
mod project_store;
mod proxies;
mod reframe_analysis;
mod reframe_dialog;
mod remix_dialog;
mod rename_clip;
mod render_previews;
mod render_replace;
mod replace_clip;
mod safe_margins;
mod scene_edits;
mod scopes;
mod sequence_marks;
mod sequence_menu;
mod sequence_settings;
mod sequence_tabs;
mod shortcuts;
mod source;
mod speed_dialog;
mod theme;
mod timeline_view;
mod transport;
mod trim_monitor;
mod viewer;
mod workspaces;

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
