use crate::editor::Editor;
use crate::import::Importer;

impl Editor {
    /// Drops what refers to the closed project's clips and items by id: the new project
    /// reuses the same ids for different things.
    pub(crate) fn forget_project_work(&mut self) {
        self.importer = Importer::new(self.timeline.sequence().timebase)
            .map_err(|error| self.notices.push(format!("Import disabled: {error}")))
            .ok();
        self.clipboard = None;
        self.nest_trail.clear();
        self.sequence_tabs.clear();
        self.pool.forget_items();
        self.proxy_offer.forget_pending();
        // Dropping a running render cancels it.
        self.render_replace = None;
        self.effects.forget_clips();
        self.breakouts.clear();
        self.view.cancel_drag();
        self.view.selection.clear();
        self.text_panel = Default::default();
        self.captions = Default::default();
        self.view.edit_point = None;
        self.view.gap = None;
        self.cancel_transcribing();
        self.essential_sound = Default::default();
        self.clip_sync = None;
        self.multicam_sync = None;
        self.scene_job = None;
        self.motion_job = None;
        self.reframe_jobs = Default::default();
        self.tone_analysis = None;
        self.mask_track_job = None;
        self.gain_dialog = None;
        self.duck_dialog = None;
        self.pauses_dialog = None;
        self.beats_dialog = None;
        self.remix_dialog = None;
        self.transition_duration_dialog = None;
        self.interpret_dialog = None;
        self.channels_dialog = None;
        self.timecode_dialog = None;
        self.subclip_dialog = None;
        self.attributes_dialog = None;
        self.reframe_dialog = None;
        self.speed_prompt = None;
        self.clip_rename = None;
        self.marker_dialog = None;
        self.fit_clip = None;
        // Its automation modes and passes are for the closed project's tracks.
        self.mixer = Default::default();
    }
}
