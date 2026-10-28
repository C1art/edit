use std::sync::Arc;

use audio::AudioOutput;
use eframe::egui_wgpu::RenderState;
use egui::{TextureId, Vec2};
use playback::{PlaybackError, Player};
use render::Scope;
use timeline::Sequence;

use crate::monitor::Monitor;

/// A player and the monitor that shows its frames.
pub struct Viewer {
    pub player: Player,
    monitor: Option<Monitor>,
}

impl Viewer {
    pub fn new(
        sequence: Arc<Sequence>,
        audio: Option<&AudioOutput>,
        render_state: Option<&RenderState>,
    ) -> Result<Self, PlaybackError> {
        Ok(Self {
            player: Player::new(sequence, audio.map(AudioOutput::writer))?,
            monitor: render_state.cloned().map(Monitor::new),
        })
    }

    /// Collects playback output and uploads a changed frame. Returns problems to report.
    pub fn update(&mut self, ctx: &egui::Context) -> Vec<String> {
        let mut notices: Vec<String> = self
            .player
            .take_warnings()
            .iter()
            .map(ToString::to_string)
            .collect();
        if self.player.update() {
            let format = self.player.sequence().format;
            if let (Some(monitor), Some(frame)) = (&mut self.monitor, self.player.current_frame()) {
                if let Err(error) = monitor.present(frame, (format.width, format.height)) {
                    notices.push(format!("Cannot display frame: {error}"));
                }
            }
        }
        if self.player.is_busy() {
            ctx.request_repaint();
        }
        notices
    }

    pub fn set_scope(&mut self, scope: Option<Scope>) {
        if let Some(monitor) = &mut self.monitor {
            monitor.set_scope(scope);
        }
    }

    pub fn scope(&self) -> Option<TextureId> {
        self.monitor.as_ref().and_then(Monitor::scope)
    }

    pub fn picture(&self, ui: &mut egui::Ui, size: Vec2) -> egui::Response {
        match &self.monitor {
            Some(monitor) => monitor.ui(ui, size),
            None => {
                let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
                ui.painter().rect_filled(rect, 0.0, egui::Color32::BLACK);
                response
            }
        }
    }
}
