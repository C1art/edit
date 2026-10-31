//! What the editor does each frame: collect finished work, keep the monitors in step
//! with the timeline, then lay out the panels and dialogs.

use std::time::Duration;

use crate::editor::Editor;
use crate::import::Importer;

impl Editor {
    pub(crate) fn sync_program(&mut self) {
        let revision = self.timeline.revision();
        if revision == self.shown_revision {
            return;
        }
        self.shown_revision = revision;
        self.prune_nest_trail();
        self.prune_sequence_tabs();
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
            viewer.player.set_scrub_audio(self.scrub_audio);
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
        self.poll_store();
        self.autosave_tick();
        self.keep_preferences();
        self.tend_audio_output(&ctx);
        self.poll_imports();
        self.poll_offline();
        self.poll_render_previews(&ctx);
        self.poll_render_replace(&ctx);
        crate::essential_sound::poll(self, &ctx);
        self.poll_proxies(&ctx);
        self.poll_multicam_sync(&ctx);
        self.poll_clip_sync(&ctx);
        self.poll_scene_edits(&ctx);
        self.poll_motion_analysis(&ctx);
        self.poll_reframe_analysis(&ctx);
        self.poll_tone_analysis(&ctx);
        self.poll_mask_tracking(&ctx);
        self.poll_export_frame(&ctx);
        self.poll_export_queue(&ctx);
        self.poll_transcribing(&ctx);
        // Every frame, as the project panel and source monitor draw from it too, with
        // or without the timeline showing.
        self.clip_art.poll(&ctx);
        if self.graphics.poll(&ctx) {
            ctx.request_repaint();
        }
        if self.vst.poll() {
            ctx.request_repaint();
        }
        crate::vst::tend_editors(self);
        if self.importer.as_ref().is_some_and(Importer::is_busy) {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        // Panels edit after the sync below, and a key pressed in the next frame, such as
        // End, must already see that edit.
        self.sync_program();
        crate::transcribe::take_keys(self, &ctx);
        crate::shortcuts::handle(self, &ctx);
        self.end_stopped_dynamic_trim();
        crate::mixer::finish_stopped_passes(self);
        self.update_window();
        self.sync_program();
        self.update_viewers(&ctx);
        self.follow_gang();
        self.follow_breakouts(&ctx);
        self.follow_playhead_with_selection();

        self.lay_out_panels(ui);

        crate::dialogs::show(self, &ctx);
    }
}
