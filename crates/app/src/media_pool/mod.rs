mod actions;
mod item_menu;
pub(crate) mod remove;
mod rows;
mod search_bins;
mod tiles;

use std::iter;

use egui::{CollapsingHeader, DragAndDrop, Response, RichText, Sense, StrokeKind};
use timeline::{Bin, BinId, BinSearch, MediaItem, Sequence, SourceId};

use crate::clip_art::ClipArt;
use crate::icons::{self, Icon, Look};
use crate::multicam_sync::MulticamSync;
use crate::proxies::ProxyMaker;
use crate::theme;
use rows::{item_row, rename_field, Field};
use tiles::{tile, PoolLayout, PoolSort};

const HEADER_BUTTON: egui::Vec2 = egui::vec2(24.0, 22.0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoolAction {
    Open(SourceId),
    /// Opens a nested sequence in the timeline.
    OpenNest(SourceId),
    Remove(SourceId),
    Duplicate(SourceId),
    Rename(SourceId, String),
    /// Asks for a file to replace the item's.
    Replace(SourceId),
    /// Stops reading the items' files until they are linked again.
    MakeOffline(Vec<SourceId>),
    CreateProxy(SourceId),
    DetachProxy(SourceId),
    /// Asks for the frame rate to play the item's pictures at.
    Interpret(SourceId),
    /// Asks which of the file's channels play on each side.
    AudioChannels(SourceId),
    Timecode(SourceId),
    /// Adds a mono item for each of the file's channels.
    BreakoutToMono(SourceId),
    /// Asks for files to import.
    Import,
    /// Adds a bin inside the given one, or at the top, and starts naming it.
    NewBin(Option<BinId>),
    /// Adds a search bin at the top of the project and starts naming it.
    NewSearchBin(BinSearch),
    SetBinSearch(BinId, BinSearch),
    RenameBin(BinId, String),
    RemoveBin(BinId),
    Move(PoolEntry, Option<BinId>),
    /// Makes a multi-camera source sequence of the items, each a camera.
    CreateMulticam(Vec<SourceId>, MulticamSync),
    /// Merges a picture item with sound items, and whether to drop the picture's sound.
    Merge(Vec<SourceId>, MulticamSync, bool),
    /// Makes a sequence holding all of the item and opens it.
    NewSequenceFrom(SourceId),
    RevealInExplorer(SourceId),
    /// Opens the item's file in the application the system opens it with.
    EditOriginal(SourceId),
    /// Asks how to lay the selected items into the sequence.
    Automate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolEntry {
    Media(SourceId),
    Bin(BinId),
}

/// The project panel's search text, its selected items, the entry being renamed with
/// its name so far, and how items are laid out and ordered.
#[derive(Debug, Default)]
pub struct PoolView {
    search: String,
    pub(crate) selected: Vec<SourceId>,
    renaming: Option<(PoolEntry, String)>,
    layout: PoolLayout,
    sort: PoolSort,
    remove_camera_audio: bool,
    /// How tall a plain list row came out, the space kept for rows scrolled away.
    row_height: Option<f32>,
    /// An item in use, waiting on the user to confirm removing it with its clips.
    removing: Option<SourceId>,
    /// An item to open the bins around and scroll to.
    revealing: Option<SourceId>,
}

impl PoolView {
    /// Selects `source` alone and brings it into view.
    pub(crate) fn reveal(&mut self, source: SourceId) {
        self.selected = vec![source];
        self.revealing = Some(source);
        self.search.clear();
    }

    /// The selected items in the order the panel lists them.
    pub(crate) fn selected_in_sort_order(&self, sequence: &Sequence) -> Vec<SourceId> {
        let items = sequence.media.iter();
        let items = items.filter(|item| self.selected.contains(&item.id));
        self.sort.sorted(items).iter().map(|item| item.id).collect()
    }
}

/// Lists imported media in its bins. Click opens an item in the source monitor;
/// dragging one onto the timeline places it, and onto a bin moves it there.
pub fn ui(
    ui: &mut egui::Ui,
    view: &mut PoolView,
    sequence: &Sequence,
    importing: u64,
    proxies: Option<&ProxyMaker>,
    art: &mut ClipArt,
) -> Option<PoolAction> {
    if sequence.media.is_empty() && sequence.bins.is_empty() && importing == 0 {
        ui.label("Drop files here or use File → Import (Ctrl+I).");
    }
    let mut pool = Pool {
        sequence,
        proxies,
        art,
        view,
        action: None,
    };
    ui.horizontal(|ui| {
        pool.view_options(ui);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let new_bin = icons::button(ui, Icon::NewBin, HEADER_BUTTON)
                .on_hover_text("New Bin: add a bin at the top of the project");
            if new_bin.clicked() {
                pool.action = Some(PoolAction::NewBin(None));
            }
        });
    });
    pool.search_row(ui);
    if importing > 0 {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(match importing {
                1 => "Importing 1 file…".to_owned(),
                count => format!("Importing {count} files…"),
            });
        });
    }
    let wanted = pool.view.search.to_lowercase();
    egui::ScrollArea::vertical()
        .auto_shrink([false, true])
        .show(ui, |ui| {
            if wanted.is_empty() {
                pool.contents(ui, None);
                pool.top_drop_zone(ui);
            } else {
                let found = sequence
                    .media
                    .iter()
                    .filter(|item| item.name.to_lowercase().contains(&wanted));
                pool.items(ui, found);
            }
        });
    pool.action
}

struct Pool<'a> {
    sequence: &'a Sequence,
    proxies: Option<&'a ProxyMaker>,
    art: &'a mut ClipArt,
    view: &'a mut PoolView,
    action: Option<PoolAction>,
}

impl Pool<'_> {
    /// The bins and then the media inside `bin`, or at the top of the project.
    fn contents(&mut self, ui: &mut egui::Ui, bin: Option<BinId>) {
        let sequence = self.sequence;
        for child in sequence.bins.iter().filter(|child| child.parent == bin) {
            self.bin(ui, child);
        }
        self.items(ui, sequence.bin_items(bin));
    }

    /// `items` in the chosen order, as rows or as tiles wrapped across the panel.
    fn items<'s>(&mut self, ui: &mut egui::Ui, items: impl Iterator<Item = &'s MediaItem>) {
        let items = self.view.sort.sorted(items);
        match self.view.layout {
            PoolLayout::List => {
                for item in items {
                    self.item(ui, item);
                }
            }
            PoolLayout::Icons => {
                ui.horizontal_wrapped(|ui| {
                    for item in items {
                        self.item(ui, item);
                    }
                });
            }
        }
    }

    /// Whether the item being revealed sits in `bin` or in a bin inside it.
    fn reveals_inside(&self, bin: BinId) -> bool {
        let sequence = self.sequence;
        let Some(item) = self
            .view
            .revealing
            .and_then(|id| sequence.media.iter().find(|item| item.id == id))
        else {
            return false;
        };
        let parent = |at: &BinId| {
            let held = sequence.bins.iter().find(|held| held.id == *at)?;
            held.parent
        };
        iter::successors(item.bin, parent)
            .take(sequence.bins.len())
            .any(|at| at == bin)
    }

    fn view_options(&mut self, ui: &mut egui::Ui) {
        let view = &mut *self.view;
        let layouts = [
            (PoolLayout::List, Icon::List, "List View"),
            (
                PoolLayout::Icons,
                Icon::Tiles,
                "Icon View: hover over a picture to scrub through it",
            ),
        ];
        for (layout, icon, tip) in layouts {
            let on = view.layout == layout;
            if icons::sized_toggle(ui, on, icon, Look::Filled, HEADER_BUTTON)
                .on_hover_text(tip)
                .clicked()
            {
                view.layout = layout;
            }
        }
        let sort = icons::button(ui, Icon::Sort, HEADER_BUTTON)
            .on_hover_text(format!("Sort Items: {}", view.sort.name()));
        egui::Popup::menu(&sort).show(|ui| {
            for sort in PoolSort::ALL {
                ui.selectable_value(&mut view.sort, sort, sort.name());
            }
        });
    }

    fn bin(&mut self, ui: &mut egui::Ui, bin: &Bin) {
        let entry = PoolEntry::Bin(bin.id);
        if self.rename_in_place(ui, entry) {
            return;
        }
        let inside = Some(bin.id);
        let sequence = self.sequence;
        let count = sequence
            .bins
            .iter()
            .filter(|child| child.parent == inside)
            .count()
            + sequence.bin_items(inside).count();
        let title = RichText::new(format!("{} ({count})", bin.name)).strong();
        let shown = CollapsingHeader::new(title)
            .id_salt(("bin", bin.id.0))
            .open(self.reveals_inside(bin.id).then_some(true))
            .show(ui, |ui| self.contents(ui, inside));
        let header = shown.header_response.interact(Sense::click_and_drag());
        if bin.search.is_some() {
            let mark = egui::Rect::from_center_size(
                header.rect.right_center() + egui::vec2(10.0, 0.0),
                egui::vec2(11.0, 11.0),
            );
            icons::paint(ui.painter(), mark, Icon::Search, theme::WEAK_TEXT);
        }
        if header.drag_started() {
            header.dnd_set_drag_payload(bin.id);
        }
        if bin.search.is_none() {
            self.drop_target(ui, &header, inside);
        }
        header.context_menu(|ui| {
            match &bin.search {
                Some(search) => self.search_bin_menu(ui, bin.id, search),
                None => {
                    if ui.button("New Bin Inside").clicked() {
                        self.action = Some(PoolAction::NewBin(inside));
                    }
                }
            }
            if ui.button("Rename").clicked() {
                self.view.renaming = Some((entry, bin.name.clone()));
            }
            if bin.parent.is_some() && ui.button("Move to Top").clicked() {
                self.action = Some(PoolAction::Move(entry, None));
            }
            let delete = ui
                .button("Delete Bin")
                .on_hover_text("Its contents move to the bin around it");
            if delete.clicked() {
                self.action = Some(PoolAction::RemoveBin(bin.id));
            }
        });
    }

    fn item(&mut self, ui: &mut egui::Ui, item: &MediaItem) {
        let entry = PoolEntry::Media(item.id);
        if self.rename_in_place(ui, entry) {
            return;
        }
        let building = self.proxies.and_then(|maker| maker.progress(item.id));
        let offline = self.art.offline.contains(&item.id);
        let plain_row =
            matches!(self.view.layout, PoolLayout::List) && building.is_none() && !offline;
        let revealing = self.view.revealing == Some(item.id);
        if let Some(height) = self.view.row_height.filter(|_| plain_row && !revealing) {
            let top = ui.cursor().top();
            let shown = ui.clip_rect().y_range();
            if top > shown.max || top + height < shown.min {
                ui.allocate_space(egui::vec2(ui.available_width(), height));
                return;
            }
        }
        let timebase = self.sequence.timebase;
        let response = match self.view.layout {
            PoolLayout::List => item_row(ui, item, timebase, building, offline),
            PoolLayout::Icons => tile(ui, item, timebase, self.art, offline),
        };
        if plain_row {
            self.view.row_height = Some(response.rect.height());
        }
        if revealing {
            response.scroll_to_me(Some(egui::Align::Center));
            self.view.revealing = None;
        }
        let selected = &mut self.view.selected;
        if response.drag_started() {
            response.dnd_set_drag_payload(item.id);
            // Dragging an item outside the selection carries it alone, so it's selected
            // alone to show that.
            if !selected.contains(&item.id) {
                *selected = vec![item.id];
            }
        }
        self.drop_target(ui, &response, item.bin);
        let selected = &mut self.view.selected;
        if response.clicked() {
            if ui.input(|input| input.modifiers.command) {
                match selected.iter().position(|&id| id == item.id) {
                    Some(index) => {
                        selected.remove(index);
                    }
                    None => selected.push(item.id),
                }
            } else {
                *selected = vec![item.id];
                self.action = Some(PoolAction::Open(item.id));
            }
        }
        if selected.contains(&item.id) {
            let stroke = ui.visuals().selection.stroke;
            let rect = response.rect;
            ui.painter()
                .rect_stroke(rect, 2.0, stroke, StrokeKind::Inside);
        }
        if response.double_clicked() && item.nest.is_some() {
            self.action = Some(PoolAction::OpenNest(item.id));
        }
        response.context_menu(|ui| {
            crate::theme::scrolling_menu(ui, |ui| self.item_menu(ui, item, offline, building));
        });
    }

    /// Shows the name field in place of `entry` while it is being renamed.
    fn rename_in_place(&mut self, ui: &mut egui::Ui, entry: PoolEntry) -> bool {
        let Some((renaming, name)) = &mut self.view.renaming else {
            return false;
        };
        if *renaming != entry {
            return false;
        }
        match rename_field(ui, name) {
            Field::Editing => {}
            Field::Done => {
                let name = std::mem::take(name);
                self.action = Some(match entry {
                    PoolEntry::Media(id) => PoolAction::Rename(id, name),
                    PoolEntry::Bin(id) => PoolAction::RenameBin(id, name),
                });
                self.view.renaming = None;
            }
            Field::Cancelled => self.view.renaming = None,
        }
        true
    }

    /// The space below the list, where a drop moves things out of every bin and a click
    /// selects nothing.
    fn top_drop_zone(&mut self, ui: &mut egui::Ui) {
        let size = egui::vec2(ui.available_width(), ui.available_height().max(32.0));
        let (rect, zone) = ui.allocate_exact_size(size, Sense::click());
        if zone.clicked() {
            self.view.selected.clear();
        }
        if self.dragging_from_bin(ui.ctx()) {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Drop here to move to the top",
                egui::FontId::proportional(12.0),
                ui.visuals().weak_text_color(),
            );
        }
        self.drop_target(ui, &zone, None);
        zone.context_menu(|ui| {
            if ui.button("New Bin").clicked() {
                self.action = Some(PoolAction::NewBin(None));
            }
            if ui.button("Import…").clicked() {
                self.action = Some(PoolAction::Import);
            }
        });
    }

    fn dragging_from_bin(&self, ctx: &egui::Context) -> bool {
        let sequence = self.sequence;
        let media = DragAndDrop::payload::<SourceId>(ctx)
            .and_then(|id| sequence.media_item(*id))
            .is_some_and(|item| item.bin.is_some());
        let bin = DragAndDrop::payload::<BinId>(ctx)
            .and_then(|id| sequence.bin(*id))
            .is_some_and(|bin| bin.parent.is_some());
        media || bin
    }

    /// Moves media or a bin dropped on `response` into `to`, outlining it while one
    /// hovers; a bin is never offered a place inside itself.
    fn drop_target(&mut self, ui: &egui::Ui, response: &Response, to: Option<BinId>) {
        let fits = |bin: BinId| !to.is_some_and(|to| self.sequence.bin_within(to, bin));
        let media = response.dnd_hover_payload::<SourceId>().is_some();
        let bin = response
            .dnd_hover_payload::<BinId>()
            .is_some_and(|bin| fits(*bin));
        if !media && !bin {
            return;
        }
        let stroke = ui.visuals().selection.stroke;
        ui.painter()
            .rect_stroke(response.rect, 2.0, stroke, StrokeKind::Inside);
        // Taking a payload of the wrong type would discard it.
        let dropped = if media {
            response
                .dnd_release_payload::<SourceId>()
                .map(|id| PoolEntry::Media(*id))
        } else {
            response
                .dnd_release_payload::<BinId>()
                .map(|id| PoolEntry::Bin(*id))
        };
        if let Some(entry) = dropped {
            self.action = Some(PoolAction::Move(entry, to));
        }
    }
}
