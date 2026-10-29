use egui::{CentralPanel, Pos2, Rect};

use super::Workspaces;
use crate::editor::Editor;
use crate::{monitors, theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Pane {
    Media,
    Side,
    Timeline,
    Monitors,
}

impl Workspaces {
    /// Fills the window with the panel under `pointer`, or puts back the one that does.
    pub(super) fn toggle_maximized(&mut self, pointer: Option<Pos2>) {
        self.maximized = match self.maximized {
            Some(_) => None,
            None => pointer.and_then(|pos| {
                self.panes
                    .iter()
                    .find(|(_, rect)| rect.contains(pos))
                    .map(|&(pane, _)| pane)
            }),
        };
    }

    pub(super) fn record_panes(&mut self, panes: [(Pane, Rect); 4]) {
        self.panes = panes;
    }
}

impl Editor {
    /// As Premiere's ` key does, so a panel can be worked in at full size for a while.
    pub(crate) fn toggle_maximized_panel(&mut self) {
        self.workspaces.maximize_requested = true;
    }

    pub(super) fn show_maximized(&mut self, ui: &mut egui::Ui, pane: Pane) {
        CentralPanel::default()
            .frame(theme::dock())
            .show(ui, |ui| match pane {
                Pane::Media => theme::card(ui, |ui| self.media_panel(ui)),
                Pane::Side => theme::card(ui, |ui| self.side_panel(ui)),
                Pane::Timeline => theme::card(ui, |ui| self.timeline_panel(ui)),
                Pane::Monitors => monitors::ui(self, ui),
            });
    }
}
