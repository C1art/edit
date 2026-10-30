use crate::editor::Editor;
use crate::effects::{self, SideTab};
use crate::import::Importer;
use crate::media_pool;
use crate::mixer;
use crate::monitors::Focus;
use crate::theme;

/// What the left panel shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MediaTab {
    Project,
    MediaBrowser,
    Markers,
    History,
    Effects,
    Metadata,
}

impl MediaTab {
    pub(crate) const ALL: [Self; 6] = [
        Self::Project,
        Self::Markers,
        Self::Effects,
        Self::MediaBrowser,
        Self::Metadata,
        Self::History,
    ];

    /// How the tab is written in the workspaces file, and its title.
    pub(crate) fn words(self) -> (&'static str, &'static str) {
        match self {
            Self::Project => ("project", "Project"),
            Self::MediaBrowser => ("media_browser", "Media Browser"),
            Self::Markers => ("markers", "Markers"),
            Self::History => ("history", "History"),
            Self::Effects => ("effects", "Effects"),
            Self::Metadata => ("metadata", "Metadata"),
        }
    }
}

impl SideTab {
    pub(crate) const ALL: [Self; 8] = [
        Self::EffectControls,
        Self::LumetriColor,
        Self::AudioMixer,
        Self::ClipMixer,
        Self::EssentialSound,
        Self::Scopes,
        Self::Captions,
        Self::Text,
    ];

    /// How the tab is written in the workspaces file, and its title.
    pub(crate) fn words(self) -> (&'static str, &'static str) {
        match self {
            Self::EffectControls => ("effect_controls", "Effect Controls"),
            Self::LumetriColor => ("lumetri_color", "Lumetri Color"),
            Self::AudioMixer => ("audio_mixer", "Audio Mixer"),
            Self::ClipMixer => ("clip_mixer", "Audio Clip Mixer"),
            Self::EssentialSound => ("essential_sound", "Essential Sound"),
            Self::Scopes => ("scopes", "Scopes"),
            Self::Captions => ("captions", "Captions"),
            Self::Text => ("text", "Text"),
        }
    }
}

impl Editor {
    pub(crate) fn media_panel(&mut self, ui: &mut egui::Ui) {
        self.keep_tabs_in_mode();
        let tabs: &[MediaTab] = if self.simple() {
            &crate::ui_mode::SIMPLE_MEDIA_TABS
        } else {
            &MediaTab::ALL
        };
        theme::tab_bar(ui, &mut self.media_tab, tabs.iter().copied(), |tab| {
            tab.words().1
        });
        ui.separator();
        match self.media_tab {
            MediaTab::Project => {
                let importing = self.importer.as_ref().map_or(0, Importer::remaining);
                let sequence = self.timeline.project();
                let proxies = self.proxies.as_ref();
                let art = &mut self.clip_art;
                if let Some(action) =
                    media_pool::ui(ui, &mut self.pool, sequence, importing, proxies, art)
                {
                    self.pool_action(action);
                }
            }
            MediaTab::MediaBrowser => crate::media_browser::ui(self, ui),
            MediaTab::Markers => crate::markers_panel::ui(self, ui),
            MediaTab::History => crate::history_panel::ui(self, ui),
            MediaTab::Effects => crate::effects_panel::ui(self, ui),
            MediaTab::Metadata => crate::metadata_panel::ui(self, ui),
        }
    }

    pub(crate) fn side_panel(&mut self, ui: &mut egui::Ui) {
        self.keep_tabs_in_mode();
        let tabs: &[SideTab] = if self.simple() {
            &crate::ui_mode::SIMPLE_SIDE_TABS
        } else {
            &SideTab::ALL
        };
        theme::tab_bar(ui, &mut self.side_tab, tabs.iter().copied(), |tab| {
            tab.words().1
        });
        ui.separator();
        // Measuring costs GPU time, so only while someone is looking.
        if let (Some(program), false) = (&mut self.program, self.side_tab == SideTab::Scopes) {
            program.set_scope(None);
        }
        match self.side_tab {
            SideTab::EffectControls => effects::ui(self, ui),
            SideTab::LumetriColor => effects::lumetri_ui(self, ui),
            SideTab::AudioMixer => mixer::ui(self, ui),
            SideTab::ClipMixer => mixer::clip_mixer::ui(self, ui),
            SideTab::EssentialSound => crate::essential_sound::ui(self, ui),
            SideTab::Scopes => crate::scopes::ui(self, ui),
            SideTab::Captions => crate::captions::ui(self, ui),
            SideTab::Text => crate::transcribe::ui(self, ui),
        }
    }

    pub(crate) fn timeline_panel(&mut self, ui: &mut egui::Ui) {
        // Working in the timeline points the transport keys at the program, so End after
        // a click there goes to the sequence's end and not the source clip's.
        let pressed = ui.input(|input| input.pointer.any_pressed());
        if pressed && ui.rect_contains_pointer(ui.max_rect()) {
            self.focus = Focus::Program;
        }
        crate::menus::timeline_toolbar(self, ui);
        let playhead = self.playhead();
        let playing = self
            .program
            .as_ref()
            .is_some_and(|program| program.player.is_playing());
        self.clip_art.poll(ui.ctx());
        let carried = crate::placing::carried_media(self, ui.ctx());
        let output = self.view.ui(
            ui,
            &mut self.timeline,
            &mut self.clip_art,
            playhead,
            playing,
            &carried,
        );
        if let Some(response) = output.clip_menu {
            response.context_menu(|ui| {
                theme::scrolling_menu(ui, |ui| {
                    if self.simple() {
                        crate::simple_menus::clip_context(self, ui);
                    } else {
                        crate::clip_menu::context(self, ui);
                    }
                });
            });
        }
        if let Some(response) = output.gap_menu {
            response.context_menu(|ui| {
                if ui.button("Ripple Delete").clicked() {
                    self.ripple_delete();
                }
            });
        }
        for error in output.errors {
            self.notice(format!("Cannot edit: {error}"));
        }
        if let Some(position) = output.seek {
            self.focus = Focus::Program;
            self.seek_program(position);
        }
        if let Some(position) = output.scrub {
            self.focus = Focus::Program;
            self.scrub_program(position);
        }
        if let Some(display) = output.time_display {
            self.set_time_display(display);
        }
        if let Some(id) = output.edit_marker {
            self.edit_marker(id);
        }
        if let Some(id) = output.clear_marker {
            self.clear_marker(id);
        }
        if let Some(drop) = output.drop {
            self.focus = Focus::Program;
            crate::placing::drop_media(self, drop);
        }
        if let Some(drop) = output.effect_drop {
            crate::effects_panel::drop_effect(self, drop);
        }
        if let Some(clip) = output.open_clip {
            self.open_clip(clip);
        }
        if let Some((clip, edge, longest)) = output.transition_duration {
            self.open_transition_duration(clip, edge, longest);
        }
    }
}
