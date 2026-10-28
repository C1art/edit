use egui::RichText;

use crate::editor::Editor;
use crate::effects::{self, SideTab};
use crate::import::Importer;
use crate::media_pool;
use crate::mixer;
use crate::monitors::Focus;

/// What the left panel shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MediaTab {
    Project,
    Markers,
    History,
    Effects,
}

impl Editor {
    pub(crate) fn media_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for (tab, name) in [
                (MediaTab::Project, "Project"),
                (MediaTab::Markers, "Markers"),
                (MediaTab::Effects, "Effects"),
                (MediaTab::History, "History"),
            ] {
                ui.selectable_value(&mut self.media_tab, tab, RichText::new(name).heading());
            }
        });
        ui.separator();
        match self.media_tab {
            MediaTab::Project => {
                let importing = self.importer.as_ref().is_some_and(Importer::is_busy);
                let sequence = self.timeline.project();
                let proxies = self.proxies.as_ref();
                let art = &mut self.clip_art;
                if let Some(action) =
                    media_pool::ui(ui, &mut self.pool, sequence, importing, proxies, art)
                {
                    self.pool_action(action);
                }
            }
            MediaTab::Markers => crate::markers_panel::ui(self, ui),
            MediaTab::History => crate::history_panel::ui(self, ui),
            MediaTab::Effects => crate::effects_panel::ui(self, ui),
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

    pub(crate) fn side_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            let tabs = [
                (SideTab::EffectControls, "Effect Controls"),
                (SideTab::AudioMixer, "Audio Mixer"),
                (SideTab::EssentialSound, "Essential Sound"),
                (SideTab::Scopes, "Scopes"),
                (SideTab::Captions, "Captions"),
            ];
            for (tab, name) in tabs {
                ui.selectable_value(&mut self.side_tab, tab, RichText::new(name).heading());
            }
        });
        ui.separator();
        // Measuring costs GPU time, so only while someone is looking.
        if let (Some(program), false) = (&mut self.program, self.side_tab == SideTab::Scopes) {
            program.set_scope(None);
        }
        match self.side_tab {
            SideTab::EffectControls => effects::ui(self, ui),
            SideTab::AudioMixer => mixer::ui(self, ui),
            SideTab::EssentialSound => crate::essential_sound::ui(self, ui),
            SideTab::Scopes => crate::scopes::ui(self, ui),
            SideTab::Captions => crate::captions::ui(self, ui),
        }
    }

    pub(crate) fn timeline_panel(&mut self, ui: &mut egui::Ui) {
        crate::menus::timeline_toolbar(self, ui);
        let playhead = self.playhead();
        let playing = self
            .program
            .as_ref()
            .is_some_and(|program| program.player.is_playing());
        self.clip_art.poll(ui.ctx());
        let output = self.view.ui(
            ui,
            &mut self.timeline,
            &mut self.clip_art,
            playhead,
            playing,
        );
        if let Some(response) = output.clip_menu {
            response.context_menu(|ui| crate::clip_menu::context(self, ui));
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
        if let Some(id) = output.edit_marker {
            self.edit_marker(id);
        }
        if let Some(id) = output.clear_marker {
            self.clear_marker(id);
        }
        if let Some(drop) = output.drop {
            crate::placing::drop_media(self, drop);
        }
        if let Some(drop) = output.effect_drop {
            crate::effects_panel::drop_effect(self, drop);
        }
        if let Some(clip) = output.open_clip {
            self.open_clip(clip);
        }
    }
}
