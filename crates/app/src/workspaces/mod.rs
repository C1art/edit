mod file;
mod maximize;
mod menu;

use egui::{CentralPanel, Frame, Margin, Panel, Rect, Vec2};

use crate::editor::Editor;
use crate::effects::SideTab;
use crate::monitors;
use crate::panels::MediaTab;
use crate::project_store::Setting;
use crate::theme;
use maximize::Pane;

pub(crate) use menu::{dialog, window_menu, workspace_bar, NamingWorkspace};

/// The room under the menu bar of the window the built-in layouts were drawn for.
const REFERENCE: Vec2 = Vec2::new(1600.0, 870.0);

/// Where the panels sit, as shares of the room under the menu bar so that they keep
/// their proportions on any screen, and which tabs they show.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Layout {
    media_share: f32,
    side_share: f32,
    timeline_share: f32,
    media_tab: MediaTab,
    side_tab: SideTab,
}

impl Layout {
    /// Sizes in points at the reference window size.
    const fn new(sizes: [f32; 3], media_tab: MediaTab, side_tab: SideTab) -> Self {
        let [media_width, side_width, timeline_height] = sizes;
        Self {
            media_share: media_width / REFERENCE.x,
            side_share: side_width / REFERENCE.x,
            timeline_share: timeline_height / REFERENCE.y,
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
            SideTab::LumetriColor,
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
    /// The room the panels had last frame; when it changes they are fitted to it again.
    room: Vec2,
    /// Where each panel was last frame, to know which one the pointer is over.
    panes: [(Pane, Rect); 4],
    maximized: Option<Pane>,
    maximize_requested: bool,
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
            room: Vec2::ZERO,
            panes: [Pane::Media, Pane::Side, Pane::Timeline, Pane::Monitors]
                .map(|pane| (pane, Rect::NOTHING)),
            maximized: None,
            maximize_requested: false,
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
        self.workspaces.maximized = None;
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
        let menu_bar = Frame::new()
            .fill(theme::GUTTER)
            .inner_margin(Margin::symmetric(8, 4));
        Panel::top("menu_bar")
            .frame(menu_bar)
            .show_separator_line(false)
            .show(ui, |ui| crate::menus::menu_bar(self, ui));
        if std::mem::take(&mut self.workspaces.maximize_requested) {
            let pointer = ui.input(|input| input.pointer.latest_pos());
            self.workspaces.toggle_maximized(pointer);
        }
        if let Some(pane) = self.workspaces.maximized {
            self.show_maximized(ui, pane);
            return;
        }
        let layout = self.workspaces.current();
        let generation = self.workspaces.generation;
        let room = ui.available_rect_before_wrap().size();
        // egui keeps a panel's size as the window changes, which would squeeze the
        // monitors on a smaller screen, so the panels are set to their shares again.
        let refit = room != self.workspaces.room;
        self.workspaces.room = room;
        let sized = |panel: Panel, size: f32| {
            let panel = panel
                .resizable(true)
                .frame(theme::dock())
                .show_separator_line(false);
            if refit {
                panel.exact_size(size)
            } else {
                panel.default_size(size)
            }
        };
        let media = sized(
            Panel::left(("media_pool", generation)),
            layout.media_share * room.x,
        )
        .show(ui, |ui| theme::card(ui, |ui| self.media_panel(ui)));
        let side = sized(
            Panel::right(("side_panel", generation)),
            layout.side_share * room.x,
        )
        .show(ui, |ui| theme::card(ui, |ui| self.side_panel(ui)));
        let timeline = sized(
            Panel::bottom(("timeline", generation)),
            layout.timeline_share * room.y,
        )
        .show(ui, |ui| theme::card(ui, |ui| self.timeline_panel(ui)));
        let monitors = CentralPanel::default()
            .frame(theme::dock())
            .show(ui, |ui| monitors::ui(self, ui));
        self.workspaces.record_panes([
            (Pane::Media, media.response.rect),
            (Pane::Side, side.response.rect),
            (Pane::Timeline, timeline.response.rect),
            (Pane::Monitors, monitors.response.rect),
        ]);

        // Rounded so that a size that hasn't moved doesn't count as a change.
        let share = |size: f32, of: f32| (size / of.max(1.0) * 1e4).round() / 1e4;
        let measured = Layout {
            media_share: share(media.response.rect.width(), room.x),
            side_share: share(side.response.rect.width(), room.x),
            timeline_share: share(timeline.response.rect.height(), room.y),
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
