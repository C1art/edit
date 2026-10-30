use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use audio::AudioOutput;
use eframe::egui_wgpu::RenderState;
use timeline::{
    Clipboard, Edit, Marker, Sequence, SetSequenceFormat, SourceId, Ticks, Timebase, Timeline,
    TimelineError, VideoFormat,
};

use crate::autosave::Autosave;
use crate::captions::CaptionPanel;
use crate::clip_art::{ClipArt, ThumbnailWorker, WaveformWorker};
use crate::effects::{EffectControls, SideTab};
use crate::export::ExportDialog;
use crate::import::Importer;
use crate::media_pool;
use crate::mixer::Mixer;
use crate::monitors::Focus;
use crate::nesting::Level;
use crate::panels::MediaTab;
use crate::path_dialog::FolderUse;
use crate::project_files::{is_project, is_subrip, is_xml, AfterDiscard, PathPrompt};
use crate::project_store::{ProjectStore, Setting};
use crate::proxies::ProxyMaker;
use crate::recent_projects::RecentProjects;
use crate::source::SourceMonitor;
use crate::timeline_view::TimelineView;
use crate::trim_monitor::TrimMonitor;
use crate::viewer::Viewer;
use crate::window_state::WindowState;

/// Notices kept for display; older ones are dropped.
const MAX_NOTICES: usize = 8;

pub struct Editor {
    pub(crate) timeline: Timeline,
    pub(crate) audio: Option<AudioOutput>,
    pub(crate) audio_switch: crate::audio_switch::AudioSwitch,
    pub(crate) render_state: Option<RenderState>,
    pub(crate) importer: Option<Importer>,
    pub(crate) proxies: Option<ProxyMaker>,
    pub(crate) proxy_offer: crate::proxy_offer::ProxyOffer,
    pub(crate) voiceover: crate::voiceover::Voiceover,
    pub(crate) offline: Option<crate::offline::OfflineMedia>,
    pub(crate) render_previews: Option<crate::render_previews::RenderPreviews>,
    pub(crate) render_replace: Option<crate::render_replace::RenderReplace>,
    pub(crate) clip_art: ClipArt,
    /// Whether the monitors play proxies where media has them.
    pub(crate) use_proxies: bool,
    /// Whether dragging the playhead and stepping frames sound the audio there.
    pub(crate) scrub_audio: bool,
    pub(crate) selection_follows_playhead: bool,
    /// Where the playhead was when the selection last followed it.
    pub(crate) selection_followed: Option<Ticks>,
    /// Outlines the action and title safe areas over the program picture.
    pub(crate) safe_margins: bool,
    pub(crate) transparency_grid: bool,
    pub(crate) ganged: bool,
    pub(crate) gang: Option<crate::gang::Gang>,
    /// Items waiting on their files to say how many channels to break out.
    pub(crate) breakouts: Vec<timeline::SourceId>,
    pub(crate) dropped_frame_indicator: bool,
    pub(crate) full_screen: bool,
    pub(crate) transition_defaults: crate::transition_defaults::TransitionDefaults,
    pub(crate) still_length: crate::still_length::StillLength,
    pub(crate) media_scaling: crate::media_scaling::MediaScaling,
    pub(crate) program_zoom: crate::program_zoom::ProgramZoom,
    /// Whether the program monitor shows every camera of the multi-camera clips beside
    /// the program, and where it puts them while it does.
    pub(crate) multicam_view: bool,
    pub(crate) multicam_layout: Option<timeline::MulticamLayout>,
    pub(crate) multicam_sync: Option<crate::multicam_sync::SyncJob>,
    pub(crate) scene_job: Option<crate::scene_edits::SceneJob>,
    pub(crate) still_job: Option<playback::StillJob>,
    pub(crate) motion_job: Option<crate::motion_analysis::MotionJob>,
    pub(crate) reframe_jobs: crate::reframe_analysis::ReframeJobs,
    pub(crate) tone_analysis: Option<crate::color_match::ToneAnalysis>,
    pub(crate) mask_track_job: Option<crate::mask_tracking::MaskTrackJob>,
    pub(crate) hardware_decoding: bool,
    pub(crate) source: Option<SourceMonitor>,
    pub(crate) left_sources: HashMap<SourceId, crate::source::LeftAt>,
    pub(crate) program: Option<Viewer>,
    /// Shown in place of the program picture while an edit is dragged; made on first use.
    pub(crate) trim_monitor: Option<TrimMonitor>,
    /// A reference frame shown beside the program picture, while comparing.
    pub(crate) comparison: Option<crate::comparison::Comparison>,
    /// The timeline revision the program viewer is playing.
    pub(crate) shown_revision: u64,
    pub(crate) focus: Focus,
    pub(crate) view: TimelineView,
    pub(crate) mixer: Mixer,
    pub(crate) master_meter: crate::audio_meters::MasterMeter,
    pub(crate) effects: EffectControls,
    pub(crate) essential_sound: crate::essential_sound::EssentialSound,
    pub(crate) captions: CaptionPanel,
    pub(crate) side_tab: SideTab,
    pub(crate) media_tab: MediaTab,
    pub(crate) effects_search: String,
    /// The history step last scrolled into view, so the list follows new edits only.
    pub(crate) history_scrolled: Option<usize>,
    pub(crate) marker_filter: crate::markers_panel::MarkerFilter,
    pub(crate) pool: media_pool::PoolView,
    pub(crate) metadata: crate::metadata_panel::MetadataPanel,
    pub(crate) gain_dialog: Option<crate::gain_dialog::GainDialog>,
    pub(crate) duck_dialog: Option<crate::duck_dialog::DuckDialog>,
    pub(crate) remix_dialog: Option<crate::remix_dialog::RemixDialog>,
    pub(crate) interpret_dialog: Option<crate::interpret_dialog::InterpretDialog>,
    pub(crate) channels_dialog: Option<crate::channels_dialog::ChannelsDialog>,
    pub(crate) find: crate::find_dialog::FindDialog,
    pub(crate) project_manager: Option<crate::project_manager::ProjectManager>,
    pub(crate) sequence_settings: Option<crate::sequence_settings::SequenceSettings>,
    pub(crate) automate_dialog: Option<crate::automate_dialog::AutomateDialog>,
    pub(crate) reframe_dialog: Option<crate::reframe_dialog::ReframeDialog>,
    /// An edit from the source waiting on how to fit its clip to the sequence marks.
    pub(crate) fit_clip: Option<timeline::PlaceMode>,
    pub(crate) attributes_dialog: Option<crate::attributes_dialog::AttributesDialog>,
    pub(crate) loudness: Option<crate::loudness_window::LoudnessWindow>,
    pub(crate) speed_prompt: Option<crate::speed_dialog::SpeedPrompt>,
    pub(crate) clip_rename: Option<crate::rename_clip::ClipRename>,
    pub(crate) clipboard: Option<Clipboard>,
    /// A copy of the marker being edited, while the marker dialog is showing.
    pub(crate) marker_dialog: Option<(timeline::MarkerOwner, Marker)>,
    pub(crate) path_prompt: Option<PathPrompt>,
    pub(crate) browse_folders: HashMap<FolderUse, PathBuf>,
    pub(crate) recent_projects: RecentProjects,
    pub(crate) window: WindowState,
    pub(crate) store: Option<ProjectStore>,
    /// What was last read or written, or nothing before the first frame.
    pub(crate) saved_preferences: Option<crate::preferences::Preferences>,
    /// Where the project was last saved or opened from.
    pub(crate) project_path: Option<PathBuf>,
    /// The timeline revision on disk; any other means unsaved changes.
    pub(crate) recovery: crate::recovery::Recovery,
    pub(crate) saved_revision: u64,
    pub(crate) confirm_discard: Option<AfterDiscard>,
    /// Runs once the save in progress succeeds.
    pub(crate) after_save: Option<AfterDiscard>,
    /// Set once the user chose to close, so the close request isn't questioned again.
    pub(crate) quitting: bool,
    pub(crate) autosave: Autosave,
    pub(crate) window_title: String,
    pub(crate) ctx: egui::Context,
    pub(crate) export: Option<ExportDialog>,
    pub(crate) export_queue: crate::export_queue::ExportQueue,
    /// The sequences left to reach the open nest, outermost first.
    pub(crate) nest_trail: Vec<Level>,
    pub(crate) sequence_tabs: Vec<crate::sequence_tabs::SequenceTab>,
    /// Messages not yet handed to `notifications`.
    pub(crate) notices: Vec<String>,
    pub(crate) notifications: crate::notifications::Notifications,
    pub(crate) keymap: crate::keymap::Keymap,
    pub(crate) keymap_window: Option<crate::keymap::KeymapWindow>,
    pub(crate) presets: Vec<timeline::EffectPreset>,
    pub(crate) preset_dialog: Option<crate::effect_presets::PresetDialog>,
    pub(crate) title_templates: Vec<timeline::TitleTemplate>,
    pub(crate) template_dialog: Option<crate::title_templates::TemplateDialog>,
    pub(crate) workspaces: crate::workspaces::Workspaces,
    pub(crate) naming_workspace: Option<crate::workspaces::NamingWorkspace>,
    pub(crate) typing: Option<crate::graphics_tools::Typing>,
}

impl Editor {
    pub fn new(
        creation_context: &eframe::CreationContext<'_>,
        initial: Vec<PathBuf>,
        window: WindowState,
    ) -> Self {
        crate::theme::apply(&creation_context.egui_ctx);
        let mut notices = Vec::new();
        let render_state = creation_context.wgpu_render_state.clone();
        if render_state.is_none() {
            notices.push("Video disabled: the window has no wgpu renderer".to_owned());
        }
        // Before anything decodes, so decoders open on the adapter that draws.
        crate::gpu_setup::decode_where_drawn(render_state.as_ref());
        let timeline = Timeline::new(Sequence::with_default_tracks(
            Timebase::HZ_90K,
            VideoFormat::default(),
        ));
        let importer = Importer::new(timeline.sequence().timebase)
            .map_err(|error| notices.push(format!("Import disabled: {error}")))
            .ok();
        let offline = crate::offline::OfflineMedia::new()
            .map_err(|error| notices.push(format!("Offline media checks disabled: {error}")))
            .ok();
        let render_previews = crate::render_previews::RenderPreviews::new()
            .map_err(|error| notices.push(format!("Render previews disabled: {error}")))
            .ok();
        let proxies = ProxyMaker::new()
            .map_err(|error| notices.push(format!("Proxies disabled: {error}")))
            .ok();
        let clip_art = ClipArt::new(
            WaveformWorker::new()
                .map_err(|error| notices.push(format!("Waveforms disabled: {error}")))
                .ok(),
            ThumbnailWorker::new()
                .map_err(|error| notices.push(format!("Thumbnails disabled: {error}")))
                .ok(),
        );
        let mut store = ProjectStore::new()
            .map_err(|error| notices.push(format!("Saving disabled: {error}")))
            .ok();
        if let Some(store) = &mut store {
            Setting::READ_AT_START
                .into_iter()
                .for_each(|setting| store.read_setting(setting));
            store.find_recoverable();
        }
        let program = Viewer::new(
            Arc::new(timeline.sequence().clone()),
            None,
            render_state.as_ref(),
        )
        .map_err(|error| notices.push(format!("Playback disabled: {error}")))
        .ok();
        let mut editor = Self {
            shown_revision: timeline.revision(),
            recovery: Default::default(),
            saved_revision: timeline.revision(),
            timeline,
            audio: None,
            audio_switch: Default::default(),
            render_state,
            importer,
            proxies,
            proxy_offer: Default::default(),
            voiceover: Default::default(),
            offline,
            render_previews,
            render_replace: None,
            clip_art,
            use_proxies: false,
            scrub_audio: true,
            selection_follows_playhead: false,
            selection_followed: None,
            safe_margins: false,
            transparency_grid: false,
            ganged: false,
            gang: None,
            breakouts: Vec::new(),
            dropped_frame_indicator: false,
            full_screen: false,
            transition_defaults: crate::transition_defaults::TransitionDefaults::STANDARD,
            still_length: crate::still_length::StillLength::STANDARD,
            media_scaling: crate::media_scaling::MediaScaling::None,
            program_zoom: Default::default(),
            multicam_view: false,
            multicam_layout: None,
            multicam_sync: None,
            scene_job: None,
            still_job: None,
            motion_job: None,
            reframe_jobs: Default::default(),
            tone_analysis: None,
            mask_track_job: None,
            hardware_decoding: true,
            source: None,
            left_sources: HashMap::new(),
            program,
            trim_monitor: None,
            comparison: None,
            focus: Focus::Program,
            view: TimelineView::default(),
            mixer: Mixer::default(),
            master_meter: Default::default(),
            effects: EffectControls::default(),
            essential_sound: Default::default(),
            captions: CaptionPanel::default(),
            side_tab: SideTab::default(),
            media_tab: MediaTab::Project,
            effects_search: String::new(),
            history_scrolled: None,
            marker_filter: Default::default(),
            pool: Default::default(),
            metadata: Default::default(),
            gain_dialog: None,
            duck_dialog: None,
            remix_dialog: None,
            interpret_dialog: None,
            channels_dialog: None,
            find: Default::default(),
            project_manager: None,
            sequence_settings: None,
            automate_dialog: None,
            reframe_dialog: None,
            fit_clip: None,
            attributes_dialog: None,
            loudness: None,
            speed_prompt: None,
            clip_rename: None,
            clipboard: None,
            marker_dialog: None,
            path_prompt: None,
            browse_folders: HashMap::new(),
            recent_projects: RecentProjects::default(),
            window,
            store,
            saved_preferences: None,
            project_path: None,
            confirm_discard: None,
            after_save: None,
            autosave: Autosave::default(),
            quitting: false,
            window_title: String::new(),
            ctx: creation_context.egui_ctx.clone(),
            export: None,
            export_queue: Default::default(),
            nest_trail: Vec::new(),
            sequence_tabs: Vec::new(),
            notices,
            notifications: Default::default(),
            keymap: Default::default(),
            keymap_window: None,
            presets: Vec::new(),
            workspaces: crate::workspaces::Workspaces::default(),
            naming_workspace: None,
            typing: None,
            preset_dialog: None,
            title_templates: Vec::new(),
            template_dialog: None,
        };
        for path in initial {
            editor.open_path(path);
        }
        editor
    }

    pub(crate) fn save_keymap(&mut self) {
        if let Some(store) = &mut self.store {
            store.write_setting(Setting::Keymap, self.keymap.to_text());
        }
    }

    pub(crate) fn notice(&mut self, notice: impl Into<String>) {
        self.notices.push(notice.into());
        let excess = self.notices.len().saturating_sub(MAX_NOTICES);
        self.notices.drain(..excess);
    }

    pub(crate) fn apply(&mut self, edit: impl Edit) -> bool {
        match self.timeline.apply(edit) {
            Ok(()) => true,
            Err(TimelineError::NothingToEdit) => false,
            Err(error) => {
                self.notice(format!("Cannot edit: {error}"));
                false
            }
        }
    }

    /// Applies an edit that changes only levels, or nothing that plays at all, handing it
    /// to the program player without the seek a structural edit needs, so playback
    /// carries on uninterrupted.
    pub(crate) fn apply_levels(&mut self, edit: impl Edit) {
        if !self.apply(edit) {
            return;
        }
        self.shown_revision = self.timeline.revision();
        self.mix_program(self.timeline.sequence().clone());
    }

    /// Opens a project file or imports anything else.
    pub(crate) fn open_path(&mut self, path: PathBuf) {
        if is_project(&path) {
            self.discard_then(AfterDiscard::Open(path));
        } else if is_subrip(&path) {
            self.import_captions(path);
        } else if is_xml(&path) {
            self.import_interchange(path);
        } else {
            self.import(path, None);
        }
    }

    pub(crate) fn playhead(&self) -> Ticks {
        self.program
            .as_ref()
            .map_or(Ticks::ZERO, |program| program.player.position())
    }

    pub(crate) fn scrub_program(&mut self, position: Ticks) {
        self.sync_program();
        if let Some(program) = &mut self.program {
            program.player.scrub(position);
        }
    }

    pub(crate) fn seek_program(&mut self, position: Ticks) {
        // The player clamps seeks to its sequence, which must include any edit just made.
        self.sync_program();
        if let Some(program) = &mut self.program {
            program.player.seek(position);
        }
    }

    /// An empty sequence takes on the format of the first video placed in it.
    pub(crate) fn match_format(&mut self, source: SourceId) {
        let sequence = self.timeline.sequence();
        // A nest shares the project's format, which its contents shouldn't decide.
        if sequence.duration() > Ticks::ZERO || self.timeline.open_nest().is_some() {
            return;
        }
        let Some(format) = sequence
            .media_item(source)
            .and_then(|item| item.played_format())
        else {
            return;
        };
        if format != sequence.format {
            self.apply(SetSequenceFormat { format });
        }
    }
}
