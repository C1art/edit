use std::path::PathBuf;
use std::sync::Arc;

use audio::AudioOutput;
use eframe::egui_wgpu::RenderState;
use egui::{CentralPanel, Panel};
use timeline::{
    Clipboard, Edit, Marker, Sequence, SetProxy, SetSequenceFormat, SourceId, Ticks, Timebase,
    Timeline, TimelineError, VideoFormat,
};

use crate::autosave::Autosave;
use crate::captions::CaptionPanel;
use crate::clip_art::{ClipArt, ThumbnailWorker, WaveformWorker};
use crate::effects::{EffectControls, SideTab};
use crate::export::ExportDialog;
use crate::import::Importer;
use crate::media_pool;
use crate::mixer::Mixer;
use crate::monitors::{self, Focus};
use crate::nesting::Level;
use crate::panels::MediaTab;
use crate::project_files::{is_project, is_subrip, is_xml, AfterDiscard, PathPrompt};
use crate::project_store::ProjectStore;
use crate::proxies::ProxyMaker;
use crate::source::SourceMonitor;
use crate::timeline_view::TimelineView;
use crate::viewer::Viewer;

/// Notices kept for display; older ones are dropped.
const MAX_NOTICES: usize = 8;

pub struct Editor {
    pub(crate) timeline: Timeline,
    pub(crate) audio: Option<AudioOutput>,
    pub(crate) render_state: Option<RenderState>,
    pub(crate) importer: Option<Importer>,
    pub(crate) proxies: Option<ProxyMaker>,
    pub(crate) offline: Option<crate::offline::OfflineMedia>,
    pub(crate) render_previews: Option<crate::render_previews::RenderPreviews>,
    pub(crate) clip_art: ClipArt,
    /// Whether the monitors play proxies where media has them.
    pub(crate) use_proxies: bool,
    /// Outlines the action and title safe areas over the program picture.
    pub(crate) safe_margins: bool,
    /// Whether the program monitor shows every camera of the multi-camera clips beside
    /// the program, and where it puts them while it does.
    pub(crate) multicam_view: bool,
    pub(crate) multicam_layout: Option<timeline::MulticamLayout>,
    pub(crate) multicam_sync: Option<crate::multicam_sync::SyncJob>,
    pub(crate) scene_job: Option<crate::scene_edits::SceneJob>,
    pub(crate) still_job: Option<playback::StillJob>,
    pub(crate) motion_job: Option<crate::motion_analysis::MotionJob>,
    pub(crate) hardware_decoding: bool,
    pub(crate) source: Option<SourceMonitor>,
    pub(crate) program: Option<Viewer>,
    /// The timeline revision the program viewer is playing.
    pub(crate) shown_revision: u64,
    pub(crate) focus: Focus,
    pub(crate) view: TimelineView,
    pub(crate) mixer: Mixer,
    pub(crate) effects: EffectControls,
    pub(crate) essential_sound: crate::essential_sound::EssentialSound,
    pub(crate) captions: CaptionPanel,
    pub(crate) side_tab: SideTab,
    pub(crate) media_tab: MediaTab,
    /// The history step last scrolled into view, so the list follows new edits only.
    pub(crate) history_scrolled: Option<usize>,
    pub(crate) marker_filter: crate::markers_panel::MarkerFilter,
    pub(crate) pool: media_pool::PoolView,
    pub(crate) gain_dialog: Option<crate::gain_dialog::GainDialog>,
    pub(crate) duck_dialog: Option<crate::duck_dialog::DuckDialog>,
    pub(crate) interpret_dialog: Option<crate::interpret_dialog::InterpretDialog>,
    pub(crate) sequence_settings: Option<crate::sequence_settings::SequenceSettings>,
    /// An edit from the source waiting on how to fit its clip to the sequence marks.
    pub(crate) fit_clip: Option<timeline::PlaceMode>,
    pub(crate) attributes_dialog: Option<crate::attributes_dialog::AttributesDialog>,
    pub(crate) loudness: Option<crate::loudness_window::LoudnessWindow>,
    pub(crate) speed_prompt: Option<crate::speed_dialog::SpeedPrompt>,
    pub(crate) clipboard: Option<Clipboard>,
    /// A copy of the marker being edited, while the marker dialog is showing.
    pub(crate) marker_dialog: Option<(timeline::MarkerOwner, Marker)>,
    pub(crate) path_prompt: Option<PathPrompt>,
    pub(crate) store: Option<ProjectStore>,
    /// Where the project was last saved or opened from.
    pub(crate) project_path: Option<PathBuf>,
    /// The timeline revision on disk; any other means unsaved changes.
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
    pub(crate) notices: Vec<String>,
    pub(crate) keymap: crate::keymap::Keymap,
    pub(crate) keymap_window: Option<crate::keymap::KeymapWindow>,
}

impl Editor {
    pub fn new(creation_context: &eframe::CreationContext<'_>, initial: Vec<PathBuf>) -> Self {
        let mut notices = Vec::new();
        let audio = AudioOutput::open_default()
            .map_err(|error| notices.push(format!("Audio disabled: {error}")))
            .ok();
        let render_state = creation_context.wgpu_render_state.clone();
        if render_state.is_none() {
            notices.push("Video disabled: the window has no wgpu renderer".to_owned());
        }
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
            store.read_keymap(keymap_path());
        }
        let program = Viewer::new(
            Arc::new(timeline.sequence().clone()),
            audio.as_ref(),
            render_state.as_ref(),
        )
        .map_err(|error| notices.push(format!("Playback disabled: {error}")))
        .ok();
        let mut editor = Self {
            shown_revision: timeline.revision(),
            saved_revision: timeline.revision(),
            timeline,
            audio,
            render_state,
            importer,
            proxies,
            offline,
            render_previews,
            clip_art,
            use_proxies: false,
            safe_margins: false,
            multicam_view: false,
            multicam_layout: None,
            multicam_sync: None,
            scene_job: None,
            still_job: None,
            motion_job: None,
            hardware_decoding: true,
            source: None,
            program,
            focus: Focus::Program,
            view: TimelineView::default(),
            mixer: Mixer::default(),
            effects: EffectControls::default(),
            essential_sound: Default::default(),
            captions: CaptionPanel::default(),
            side_tab: SideTab::default(),
            media_tab: MediaTab::Project,
            history_scrolled: None,
            marker_filter: Default::default(),
            pool: Default::default(),
            gain_dialog: None,
            duck_dialog: None,
            interpret_dialog: None,
            sequence_settings: None,
            fit_clip: None,
            attributes_dialog: None,
            loudness: None,
            speed_prompt: None,
            clipboard: None,
            marker_dialog: None,
            path_prompt: None,
            store,
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
            notices,
            keymap: Default::default(),
            keymap_window: None,
        };
        for path in initial {
            editor.open_path(path);
        }
        editor
    }

    pub(crate) fn save_keymap(&mut self) {
        if let Some(store) = &mut self.store {
            store.write_keymap(keymap_path(), self.keymap.to_text());
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

    pub(crate) fn open_source(&mut self, id: SourceId) {
        let Some(item) = self.timeline.sequence().media_item(id).cloned() else {
            return;
        };
        // Drop the old player first so its workers stop feeding the audio output.
        self.source = None;
        match SourceMonitor::open(
            &item,
            self.timeline.project(),
            self.audio.as_ref(),
            self.render_state.as_ref(),
        ) {
            Ok(source) => {
                self.source = Some(source);
                self.focus = Focus::Source;
            }
            Err(error) => self.notice(format!("Cannot open {}: {error}", item.name)),
        }
    }

    pub(crate) fn playhead(&self) -> Ticks {
        self.program
            .as_ref()
            .map_or(Ticks::ZERO, |program| program.player.position())
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

    fn poll_proxies(&mut self, ctx: &egui::Context) {
        let Some(maker) = &mut self.proxies else {
            return;
        };
        let finished = maker.poll();
        if maker.is_busy() {
            ctx.request_repaint();
        }
        for (source, result) in finished {
            match result {
                Ok(path) => {
                    self.apply(SetProxy {
                        source,
                        proxy: Some(path),
                    });
                }
                Err(error) => self.notice(format!("Cannot create proxy: {error}")),
            }
        }
    }

    fn sync_program(&mut self) {
        let revision = self.timeline.revision();
        if revision == self.shown_revision {
            return;
        }
        self.shown_revision = revision;
        self.prune_nest_trail();
        self.view.prune_selection(self.timeline.sequence());
        if let Some(source) = &mut self.source {
            let pool = self.timeline.project();
            if let Some(item) = pool.media_item(source.source) {
                source.refresh(item, pool);
            }
        }
        self.show_program(self.timeline.sequence().clone());
    }

    fn update_viewers(&mut self, ctx: &egui::Context) {
        let mut notices = Vec::new();
        let viewers = [
            self.program.as_mut(),
            self.source.as_mut().map(|source| &mut source.viewer),
        ];
        for viewer in viewers.into_iter().flatten() {
            viewer.player.set_proxies(self.use_proxies);
        }
        if let Some(program) = &mut self.program {
            notices.extend(program.update(ctx));
        }
        if let Some(source) = &mut self.source {
            notices.extend(source.viewer.update(ctx));
        }
        for notice in notices {
            self.notice(notice);
        }
    }
}

impl eframe::App for Editor {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let dropped: Vec<PathBuf> = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .map(|file| file.path().to_owned())
                .filter(|path| !path.as_os_str().is_empty())
                .collect()
        });
        for path in dropped {
            self.open_path(path);
        }
        self.poll_store();
        self.autosave_tick();
        self.poll_imports();
        self.poll_offline();
        self.poll_render_previews(&ctx);
        crate::essential_sound::poll(self, &ctx);
        self.poll_proxies(&ctx);
        self.poll_multicam_sync(&ctx);
        self.poll_scene_edits(&ctx);
        self.poll_motion_analysis(&ctx);
        self.poll_export_frame(&ctx);
        self.poll_export_queue(&ctx);
        if self.importer.as_ref().is_some_and(Importer::is_busy) {
            ctx.request_repaint();
        }
        crate::shortcuts::handle(self, &ctx);
        self.update_window();
        self.sync_program();
        self.update_viewers(&ctx);

        Panel::top("menu_bar").show(ui, |ui| crate::menus::menu_bar(self, ui));
        Panel::left("media_pool")
            .resizable(true)
            .default_size(260.0)
            .show(ui, |ui| self.media_panel(ui));
        Panel::right("side_panel")
            .resizable(true)
            .default_size(290.0)
            .show(ui, |ui| self.side_panel(ui));
        Panel::bottom("timeline")
            .resizable(true)
            .default_size(340.0)
            .show(ui, |ui| self.timeline_panel(ui));
        CentralPanel::default().show(ui, |ui| monitors::ui(self, ui));

        crate::project_files::path_dialog(self, &ctx);
        crate::project_files::discard_dialog(self, &ctx);
        crate::marker_dialog::dialog(self, &ctx);
        crate::gain_dialog::dialog(self, &ctx);
        crate::duck_dialog::dialog(self, &ctx);
        crate::interpret_dialog::dialog(self, &ctx);
        crate::sequence_settings::dialog(self, &ctx);
        crate::fit_clip::dialog(self, &ctx);
        crate::attributes_dialog::dialog(self, &ctx);
        crate::loudness_window::window(self, &ctx);
        crate::speed_dialog::dialog(self, &ctx);
        crate::export::dialog(self, &ctx);
        crate::export_queue::window(self, &ctx);
        crate::keymap::window(self, &ctx);
        crate::link_media::dialog(self, &ctx);
        crate::render_previews::window(self, &ctx);
    }
}

fn keymap_path() -> PathBuf {
    crate::project_store::app_folder().join("Keyboard Shortcuts.txt")
}
