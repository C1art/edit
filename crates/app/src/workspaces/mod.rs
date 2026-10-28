mod file;
mod menu;

use egui::{CentralPanel, Panel};

use crate::editor::Editor;
use crate::effects::SideTab;
use crate::monitors;
use crate::panels::MediaTab;
use crate::project_store::Setting;

pub(crate) use menu::{dialog, window_menu, workspace_bar, NamingWorkspace};

/// Where the panels sit and which tabs they show.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Layout {
    media_width: f32,
    side_width: f32,
    timeline_height: f32,
    media_tab: MediaTab,
    side_tab: SideTab,
}

impl Layout {
    const fn new(sizes: [f32; 3], media_tab: MediaTab, side_tab: SideTab) -> Self {
        let [media_width, side_width, timeline_height] = sizes;
        Self {
            media_width,
            side_width,
            timeline_height,
            media_tab,
            side_tab,
        }
    }
}

const BUILT_IN: [(&str, Layout); 6] = [
    (
        "Assembly",
        Layout::new(
            [620.0, 260.0, 280.0],
            MediaTab::Project,
            SideTab::EffectControls,
        ),
    ),
    (
        "Editing",
        Layout::new(
            [260.0, 290.0, 340.0],
            MediaTab::Project,
            SideTab::EffectControls,
        ),
    ),
    (
        "Color",
        Layout::new(
            [220.0, 380.0, 280.0],
            MediaTab::Project,
            SideTab::EffectControls,
        ),
    ),
    (
        "Effects",
        Layout::new(
            [300.0, 330.0, 340.0],
            MediaTab::Effects,
            SideTab::EffectControls,
        ),
    ),
    (
        "Audio",
        Layout::new(
            [260.0, 340.0, 380.0],
            MediaTab::Project,
            SideTab::EssentialSound,
        ),
    ),
    (
        "Captions",
        Layout::new([260.0, 360.0, 340.0], MediaTab::Project, SideTab::Captions),
    ),
];

const EDITING: usize = 1;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Workspace {
    pub(crate) name: String,
    /// What Reset to Saved Layout goes back to.
    saved: Layout,
    /// The layout as last left, which switching back restores.
    current: Layout,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Workspaces {
    /// The built-in workspaces first, in their fixed order.
    list: Vec<Workspace>,
    active: usize,
    /// Part of the panels' ids, bumped so that they take up the active layout's sizes
    /// instead of the ones egui remembers for them.
    generation: u64,
    /// The current layout changed since the file was written.
    unsaved: bool,
}

impl Default for Workspaces {
    fn default() -> Self {
        let list = BUILT_IN.map(|(name, layout)| Workspace {
            name: name.to_owned(),
            saved: layout,
            current: layout,
        });
        Self {
            list: list.into(),
            active: EDITING,
            generation: 0,
            unsaved: false,
        }
    }
}

impl Workspaces {
    pub(crate) fn list(&self) -> &[Workspace] {
        &self.list
    }

    pub(crate) fn active(&self) -> usize {
        self.active
    }

    pub(crate) fn is_built_in(index: usize) -> bool {
        index < BUILT_IN.len()
    }

    fn current(&self) -> Layout {
        self.list[self.active].current
    }

    fn current_mut(&mut self) -> &mut Layout {
        &mut self.list[self.active].current
    }
}

impl Editor {
    pub(crate) fn switch_workspace(&mut self, index: usize) {
        if index < self.workspaces.list.len() && index != self.workspaces.active {
            self.workspaces.active = index;
            self.apply_layout(self.workspaces.current());
            self.save_workspaces();
        }
    }

    pub(crate) fn reset_workspace(&mut self) {
        let workspaces = &self.workspaces;
        self.apply_layout(workspaces.list[workspaces.active].saved);
        self.save_workspaces();
    }

    pub(crate) fn save_workspace_changes(&mut self) {
        let workspace = &mut self.workspaces.list[self.workspaces.active];
        workspace.saved = workspace.current;
        self.save_workspaces();
    }

    /// Keeps the current layout under `name`, which must not already be taken.
    pub(crate) fn save_new_workspace(&mut self, name: &str) -> Result<(), String> {
        // The workspaces file is written a line to a workspace and a tab between fields.
        let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
        if name.is_empty() {
            return Err("Name the workspace".to_owned());
        }
        if self.workspaces.list.iter().any(|kept| kept.name == name) {
            return Err(format!("There is already a workspace called {name}"));
        }
        let layout = self.workspaces.current();
        self.workspaces.list.push(Workspace {
            name,
            saved: layout,
            current: layout,
        });
        self.workspaces.active = self.workspaces.list.len() - 1;
        self.save_workspaces();
        Ok(())
    }

    pub(crate) fn delete_workspace(&mut self, index: usize) {
        if Workspaces::is_built_in(index) || index >= self.workspaces.list.len() {
            return;
        }
        if index == self.workspaces.active {
            self.workspaces.active = EDITING;
            self.apply_layout(self.workspaces.current());
        } else if index < self.workspaces.active {
            self.workspaces.active -= 1;
        }
        self.workspaces.list.remove(index);
        self.save_workspaces();
    }

    pub(crate) fn load_workspaces(&mut self, text: &str) {
        let generation = self.workspaces.generation;
        self.workspaces = Workspaces::from_text(text);
        self.workspaces.generation = generation;
        self.apply_layout(self.workspaces.current());
    }

    fn apply_layout(&mut self, layout: Layout) {
        self.media_tab = layout.media_tab;
        self.side_tab = layout.side_tab;
        *self.workspaces.current_mut() = layout;
        self.workspaces.generation += 1;
    }

    fn save_workspaces(&mut self) {
        self.workspaces.unsaved = false;
        if let Some(store) = &mut self.store {
            store.write_setting(Setting::Workspaces, self.workspaces.to_text());
        }
    }

    /// Shows every panel at the active workspace's sizes, and remembers the sizes
    /// they are dragged to.
    pub(crate) fn lay_out_panels(&mut self, ui: &mut egui::Ui) {
        // First, as its menus can switch workspaces.
        Panel::top("menu_bar").show(ui, |ui| crate::menus::menu_bar(self, ui));
        let layout = self.workspaces.current();
        let generation = self.workspaces.generation;
        let media = Panel::left(("media_pool", generation))
            .resizable(true)
            .default_size(layout.media_width)
            .show(ui, |ui| self.media_panel(ui));
        let side = Panel::right(("side_panel", generation))
            .resizable(true)
            .default_size(layout.side_width)
            .show(ui, |ui| self.side_panel(ui));
        let timeline = Panel::bottom(("timeline", generation))
            .resizable(true)
            .default_size(layout.timeline_height)
            .show(ui, |ui| self.timeline_panel(ui));
        CentralPanel::default().show(ui, |ui| monitors::ui(self, ui));

        let measured = Layout {
            media_width: media.response.rect.width().round(),
            side_width: side.response.rect.width().round(),
            timeline_height: timeline.response.rect.height().round(),
            media_tab: self.media_tab,
            side_tab: self.side_tab,
        };
        if measured != layout {
            *self.workspaces.current_mut() = measured;
            self.workspaces.unsaved = true;
        }
        // Not while a divider is being dragged, which changes the sizes every frame.
        let settled = !ui.input(|input| input.pointer.any_down());
        if self.workspaces.unsaved && settled {
            self.save_workspaces();
        }
    }
}
