mod attributes_dialog;
mod audio_meters;
mod audio_switch;
mod autosave;
mod captions;
mod channels_dialog;
mod clip_art;
mod clip_menu;
mod clip_moves;
mod color_match;
mod commands;
mod comparison;
mod dialog;
mod dropped_frames;
mod duck_dialog;
mod editor;
mod effect_presets;
mod effects;
mod effects_panel;
mod error;
mod essential_sound;
mod export;
mod export_format;
mod export_frame;
mod export_progress;
mod export_queue;
mod export_summary;
mod export_target;
mod export_video;
mod file_browser;
mod file_drop;
mod find_dialog;
mod fit_clip;
mod gain_dialog;
mod gpu_setup;
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
mod monitor_drop;
mod monitors;
mod motion_analysis;
mod multicam;
mod multicam_sync;
mod nesting;
mod notifications;
mod offline;
mod panels;
mod path_dialog;
mod placing;
mod preferences;
mod program_zoom;
mod project_files;
mod project_manager;
mod project_store;
mod proxies;
mod recent_projects;
mod recovery;
mod reframe_analysis;
mod reframe_dialog;
mod remix_dialog;
mod rename_clip;
mod render_previews;
mod render_replace;
mod replace_clip;
mod safe_margins;
mod same_path;
mod scene_edits;
mod scopes;
mod sequence_marks;
mod sequence_menu;
mod sequence_settings;
mod sequence_tabs;
mod shortcuts;
#[cfg(test)]
mod smoke_tests;
mod source;
mod source_drag;
mod source_waveform;
mod speed_dialog;
mod theme;
mod timecode_field;
mod timeline_view;
mod transport;
mod trim_monitor;
mod viewer;
mod window_state;
mod workspaces;

use std::path::PathBuf;

use editor::Editor;
use window_state::WindowState;

fn main() -> eframe::Result {
    let initial: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let window = WindowState::load();
    let options = eframe::NativeOptions {
        viewport: window.open(
            egui::ViewportBuilder::default()
                .with_title("Edit")
                .with_drag_and_drop(true),
        ),
        centered: window.centered(),
        wgpu_options: gpu_setup::wgpu_configuration(),
        ..Default::default()
    };
    eframe::run_native(
        "Edit",
        options,
        Box::new(|creation_context| Ok(Box::new(Editor::new(creation_context, initial, window)))),
    )
}
