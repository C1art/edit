use std::path::PathBuf;
use std::sync::Arc;

use audio::AudioOutput;
use eframe::egui_wgpu::RenderState;
use egui::{CentralPanel, Panel, RichText};
use timeline::{
    Clipboard, Edit, Sequence, SetProxy, SetSequenceFormat, SourceId, Ticks, Timebase, Timeline,
    TimelineError, VideoFormat,
};

use crate::effects::{self, EffectControls, SideTab};
use crate::export::ExportDialog;
use crate::import::Importer;
use crate::media_pool::{self, PoolAction};
use crate::mixer::{self, Mixer};
use crate::monitors::{self, Focus};
use crate::project_files::{is_project, AfterDiscard, PathPrompt};
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
    proxies: Option<ProxyMaker>,
    /// Whether the monitors play proxies where media has them.
    pub(crate) use_proxies: bool,
    pub(crate) hardware_decoding: bool,
    pub(crate) source: Option<SourceMonitor>,
    pub(crate) program: Option<Viewer>,
    /// The timeline revision the program viewer is playing.
    pub(crate) shown_revision: u64,
    pub(crate) focus: Focus,
    pub(crate) view: TimelineView,
    pub(crate) mixer: Mixer,
    pub(crate) effects: EffectControls,
    side_tab: SideTab,
    /// The gain in dB being typed into the clip gain dialog, while it is showing.
    pub(crate) gain_prompt: Option<f32>,
    pub(crate) clipboard: Option<Clipboard>,
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
    pub(crate) window_title: String,
    pub(crate) ctx: egui::Context,
    pub(crate) export: Option<ExportDialog>,
    notices: Vec<String>,
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
        let proxies = ProxyMaker::new()
            .map_err(|error| notices.push(format!("Proxies disabled: {error}")))
            .ok();
        let store = ProjectStore::new()
            .map_err(|error| notices.push(format!("Saving disabled: {error}")))
            .ok();
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
            use_proxies: false,
            hardware_decoding: true,
            source: None,
            program,
            focus: Focus::Program,
            view: TimelineView::default(),
            mixer: Mixer::default(),
            effects: EffectControls::default(),
            side_tab: SideTab::default(),
            gain_prompt: None,
            clipboard: None,
            path_prompt: None,
            store,
            project_path: None,
            confirm_discard: None,
            after_save: None,
            quitting: false,
            window_title: String::new(),
            ctx: creation_context.egui_ctx.clone(),
            export: None,
            notices,
        };
        for path in initial {
            editor.open_path(path);
        }
        editor
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

    /// Applies an edit that changes only levels, handing it to the program player without
    /// the seek a structural edit needs, so playback carries on uninterrupted.
    pub(crate) fn apply_levels(&mut self, edit: impl Edit) {
        if !self.apply(edit) {
            return;
        }
        self.shown_revision = self.timeline.revision();
        if let Some(program) = &mut self.program {
            program
                .player
                .set_mix(Arc::new(self.timeline.sequence().clone()));
        }
    }

    /// Opens a project file or imports anything else.
    pub(crate) fn open_path(&mut self, path: PathBuf) {
        if is_project(&path) {
            self.discard_then(AfterDiscard::Open(path));
        } else {
            self.import(path);
        }
    }

    pub(crate) fn import(&mut self, path: PathBuf) {
        match &mut self.importer {
            Some(importer) => importer.request(path),
            None => self.notice(format!("Cannot import {}", path.display())),
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
            self.timeline.sequence().timebase,
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
        if sequence.duration() > Ticks::ZERO {
            return;
        }
        let Some(format) = sequence.media_item(source).and_then(|item| item.video) else {
            return;
        };
        if format != sequence.format {
            self.apply(SetSequenceFormat { format });
        }
    }

    fn poll_imports(&mut self) {
        let Some(importer) = &mut self.importer else {
            return;
        };
        for result in importer.poll() {
            match result {
                Ok(import) => {
                    if self.apply(import) {
                        if let Some(id) = self.timeline.sequence().media.last().map(|item| item.id)
                        {
                            self.open_source(id);
                        }
                    }
                }
                Err((path, error)) => {
                    self.notice(format!("Cannot import {}: {error}", path.display()));
                }
            }
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
        self.view.prune_selection(self.timeline.sequence());
        if let Some(source) = &mut self.source {
            if let Some(item) = self.timeline.sequence().media_item(source.source) {
                source.refresh(item);
            }
        }
        if let Some(program) = &mut self.program {
            program
                .player
                .set_sequence(Arc::new(self.timeline.sequence().clone()));
        }
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

    fn media_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Project");
        ui.separator();
        let importing = self.importer.as_ref().is_some_and(Importer::is_busy);
        let sequence = self.timeline.sequence();
        let proxies = self.proxies.as_ref();
        match media_pool::ui(ui, &sequence.media, sequence.timebase, importing, proxies) {
            Some(PoolAction::Open(id)) => self.open_source(id),
            Some(PoolAction::Remove(id)) => {
                if self
                    .source
                    .as_ref()
                    .is_some_and(|source| source.source == id)
                {
                    self.source = None;
                }
                self.apply(timeline::RemoveMedia { source: id });
            }
            Some(PoolAction::CreateProxy(id)) => {
                let path = sequence.media_item(id).map(|item| item.path.clone());
                if let (Some(maker), Some(path)) = (&mut self.proxies, path) {
                    maker.request(id, path);
                }
            }
            Some(PoolAction::DetachProxy(id)) => {
                self.apply(SetProxy {
                    source: id,
                    proxy: None,
                });
            }
            None => {}
        }
        if !self.notices.is_empty() {
            ui.separator();
            for notice in &self.notices {
                ui.colored_label(ui.visuals().warn_fg_color, notice);
            }
            if ui.small_button("Clear messages").clicked() {
                self.notices.clear();
            }
        }
    }

    fn side_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let tabs = [
                (SideTab::EffectControls, "Effect Controls"),
                (SideTab::AudioMixer, "Audio Mixer"),
                (SideTab::Scopes, "Scopes"),
            ];
            for (tab, name) in tabs {
                ui.selectable_value(&mut self.side_tab, tab, RichText::new(name).heading());
            }
        });
        ui.separator();
        // Measuring costs GPU time, so only while someone is looking.
        if let Some(program) = &mut self.program {
            program.set_waveform(self.side_tab == SideTab::Scopes);
        }
        match self.side_tab {
            SideTab::EffectControls => effects::ui(self, ui),
            SideTab::AudioMixer => mixer::ui(self, ui),
            SideTab::Scopes => crate::scopes::ui(self, ui),
        }
    }

    fn timeline_panel(&mut self, ui: &mut egui::Ui) {
        crate::menus::timeline_toolbar(self, ui);
        let playhead = self.playhead();
        let playing = self
            .program
            .as_ref()
            .is_some_and(|program| program.player.is_playing());
        let output = self.view.ui(ui, &mut self.timeline, playhead, playing);
        for error in output.errors {
            self.notice(format!("Cannot edit: {error}"));
        }
        if let Some(position) = output.seek {
            self.focus = Focus::Program;
            self.seek_program(position);
        }
        if let Some(drop) = output.drop {
            crate::placing::drop_media(self, drop);
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
        self.poll_imports();
        self.poll_proxies(&ctx);
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
        crate::menus::gain_dialog(self, &ctx);
        crate::export::dialog(self, &ctx);
    }
}
