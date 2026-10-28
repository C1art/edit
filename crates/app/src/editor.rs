use std::path::PathBuf;
use std::sync::Arc;

use audio::AudioOutput;
use eframe::egui_wgpu::RenderState;
use egui::{CentralPanel, Panel};
use timeline::{
    Edit, Sequence, SetSequenceFormat, SourceId, Ticks, Timebase, Timeline, TimelineError,
    VideoFormat,
};

use crate::import::Importer;
use crate::media_pool::{self, PoolAction};
use crate::monitors::{self, Focus};
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
    pub(crate) source: Option<SourceMonitor>,
    pub(crate) program: Option<Viewer>,
    /// The timeline revision the program viewer is playing.
    shown_revision: u64,
    pub(crate) focus: Focus,
    pub(crate) view: TimelineView,
    /// The path being typed into the import dialog, while it is showing.
    pub(crate) import_prompt: Option<String>,
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
        let program = Viewer::new(
            Arc::new(timeline.sequence().clone()),
            audio.as_ref(),
            render_state.as_ref(),
        )
        .map_err(|error| notices.push(format!("Playback disabled: {error}")))
        .ok();
        let mut editor = Self {
            shown_revision: timeline.revision(),
            timeline,
            audio,
            render_state,
            importer,
            source: None,
            program,
            focus: Focus::Program,
            view: TimelineView::default(),
            import_prompt: None,
            notices,
        };
        for path in initial {
            editor.import(path);
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

    fn sync_program(&mut self) {
        let revision = self.timeline.revision();
        if revision == self.shown_revision {
            return;
        }
        self.shown_revision = revision;
        self.view.prune_selection(self.timeline.sequence());
        if let Some(program) = &mut self.program {
            program
                .player
                .set_sequence(Arc::new(self.timeline.sequence().clone()));
        }
    }

    fn update_viewers(&mut self, ctx: &egui::Context) {
        let mut notices = Vec::new();
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
        match media_pool::ui(ui, &sequence.media, sequence.timebase, importing) {
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
            self.import(path);
        }
        self.poll_imports();
        if self.importer.as_ref().is_some_and(Importer::is_busy) {
            ctx.request_repaint();
        }
        crate::shortcuts::handle(self, &ctx);
        self.sync_program();
        self.update_viewers(&ctx);

        Panel::top("menu_bar").show(ui, |ui| crate::menus::menu_bar(self, ui));
        Panel::left("media_pool")
            .resizable(true)
            .default_size(260.0)
            .show(ui, |ui| self.media_panel(ui));
        Panel::bottom("timeline")
            .resizable(true)
            .default_size(340.0)
            .show(ui, |ui| self.timeline_panel(ui));
        CentralPanel::default().show(ui, |ui| monitors::ui(self, ui));

        crate::menus::import_dialog(self, &ctx);
    }
}
