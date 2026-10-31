// Released builds open no console window beside the editor's own.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod add_tracks_dialog;
mod app_icon;
mod attributes_dialog;
mod audio_meters;
mod audio_switch;
mod automate_dialog;
mod autosave;
mod beats_dialog;
mod breakout;
mod captions;
mod channels_dialog;
mod clip_art;
mod clip_menu;
mod clip_moves;
mod clip_sync;
mod color_match;
mod commands;
mod comparison;
mod dialog;
mod dialogs;
mod dropped_frames;
mod duck_dialog;
mod edit_apply;
mod editor;
mod effect_presets;
mod effects;
mod effects_panel;
mod error;
mod essential_keys;
mod essential_sound;
mod export;
mod export_captions;
mod export_format;
mod export_frame;
mod export_loudness;
mod export_overlays;
mod export_progress;
mod export_queue;
mod export_settings;
mod export_summary;
mod export_target;
mod export_video;
mod file_browser;
mod file_drop;
mod find_dialog;
mod fit_clip;
mod frame;
mod full_screen;
mod full_screen_controls;
mod gain_dialog;
mod gang;
mod gap_navigation;
mod gpu_setup;
mod graphics_tools;
mod guides;
mod history_panel;
mod icon_art;
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
mod media_browser;
mod media_pool;
mod media_scaling;
mod menus;
mod metadata_panel;
mod mixer;
mod monitor;
mod monitor_drop;
mod monitors;
mod motion_analysis;
mod multicam;
mod multicam_sync;
mod native_dialog;
mod nesting;
mod notifications;
mod offline;
mod panels;
mod path_dialog;
mod pauses_dialog;
mod placing;
mod preferences;
mod preferences_menu;
mod press_join;
mod program_zoom;
mod project_copy;
mod project_files;
mod project_manager;
mod project_open;
mod project_store;
mod project_switch;
mod proxies;
mod proxy_offer;
mod quick_export;
mod quit_exports;
mod recent_projects;
mod recovery;
mod reframe_analysis;
mod reframe_dialog;
mod remix_dialog;
mod rename_clip;
mod render_previews;
mod render_replace;
mod replace_clip;
mod reveal;
mod safe_margins;
mod same_path;
mod scene_edits;
mod scopes;
mod sequence_marks;
mod sequence_menu;
mod sequence_settings;
mod sequence_tabs;
mod shortcuts;
mod simple_menus;
mod simplify_dialog;
#[cfg(test)]
mod smoke_tests;
mod source;
mod source_drag;
mod source_waveform;
mod speed_dialog;
mod still_length;
mod subclip_dialog;
mod text_styles;
mod theme;
mod timecode_dialog;
mod timecode_field;
mod timeline_view;
mod title_templates;
mod track_follow;
mod transcribe;
mod transition_defaults;
mod transition_duration_dialog;
mod transparency_grid;
mod transport;
mod trim_monitor;
mod ui_mode;
mod unreadable_setting;
mod viewer;
mod voiceover;
mod vst;
mod window_close;
mod window_state;
mod workspaces;

use std::path::PathBuf;

use editor::Editor;
use window_state::WindowState;

fn main() -> eframe::Result {
    let initial: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    project_store::adopt_old_app_folder();
    let window = WindowState::load();
    let options = eframe::NativeOptions {
        viewport: window.open(
            egui::ViewportBuilder::default()
                .with_title("Cledit")
                .with_icon(app_icon::icon())
                .with_drag_and_drop(true),
        ),
        centered: window.centered(),
        wgpu_options: gpu_setup::wgpu_configuration(),
        ..Default::default()
    };
    let ran = eframe::run_native(
        "Cledit",
        options,
        Box::new(|creation_context| Ok(Box::new(Editor::new(creation_context, initial, window)))),
    );
    // The window is gone; a quit during an export only gets this long to clean up.
    playback::stop_exports(std::time::Duration::from_secs(5));
    ran
}
