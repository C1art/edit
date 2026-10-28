use std::collections::HashSet;

mod actions;
mod rows;

use egui::{CollapsingHeader, DragAndDrop, Response, RichText, Sense, StrokeKind, TextEdit};
use timeline::{Bin, BinId, MediaItem, Sequence, SourceId};

use crate::multicam_sync::MulticamSync;
use crate::proxies::ProxyMaker;
use rows::{item_row, proxy_menu, rename_field, Field};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoolAction {
    Open(SourceId),
    /// Opens a nested sequence in the timeline.
    OpenNest(SourceId),
    Remove(SourceId),
    Rename(SourceId, String),
    /// Asks for a file to replace the item's.
    Replace(SourceId),
    CreateProxy(SourceId),
    DetachProxy(SourceId),
    /// Asks for the frame rate to play the item's pictures at.
    Interpret(SourceId),
    /// Adds a bin inside the given one, or at the top, and starts naming it.
    NewBin(Option<BinId>),
    RenameBin(BinId, String),
    RemoveBin(BinId),
    Move(PoolEntry, Option<BinId>),
    /// Makes a multi-camera source sequence of the items, each a camera.
    CreateMulticam(Vec<SourceId>, MulticamSync),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolEntry {
    Media(SourceId),
    Bin(BinId),
}

/// The project panel's search text, its selected items, and the entry being renamed with
/// its name so far.
#[derive(Debug, Default)]
pub struct PoolView {
    search: String,
    pub(crate) selected: Vec<SourceId>,
    renaming: Option<(PoolEntry, String)>,
}

/// Lists imported media in its bins. Click opens an item in the source monitor;
/// dragging one onto the timeline places it, and onto a bin moves it there.
pub fn ui(
    ui: &mut egui::Ui,
    view: &mut PoolView,
    sequence: &Sequence,
    importing: bool,
    proxies: Option<&ProxyMaker>,
    offline: &HashSet<SourceId>,
) -> Option<PoolAction> {
    if sequence.media.is_empty() && sequence.bins.is_empty() && !importing {
        ui.label("Drop files here or use File → Import (Ctrl+I).");
    }
    let mut pool = Pool {
        sequence,
        proxies,
        offline,
        view,
        action: None,
    };
    ui.horizontal(|ui| {
        let new_bin = ui
            .button("New Bin")
            .on_hover_text("Add a bin at the top of the project");
        if new_bin.clicked() {
            pool.action = Some(PoolAction::NewBin(None));
        }
        ui.add(
            TextEdit::singleline(&mut pool.view.search)
                .hint_text("Search")
                .desired_width(f32::INFINITY),
        );
    });
    let wanted = pool.view.search.to_lowercase();
    egui::ScrollArea::vertical()
        .auto_shrink([false, true])
        .max_height(ui.available_height() * 0.6)
        .show(ui, |ui| {
            if wanted.is_empty() {
                pool.contents(ui, None);
                pool.top_drop_zone(ui);
            } else {
                let found = sequence
                    .media
                    .iter()
                    .filter(|item| item.name.to_lowercase().contains(&wanted));
                for item in found {
                    pool.item(ui, item);
                }
            }
        });
    if importing {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Importing…");
        });
    }
    pool.action
}

struct Pool<'a> {
    sequence: &'a Sequence,
    proxies: Option<&'a ProxyMaker>,
    offline: &'a HashSet<SourceId>,
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
        for item in sequence.media.iter().filter(|item| item.bin == bin) {
            self.item(ui, item);
        }
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
            + sequence
                .media
                .iter()
                .filter(|item| item.bin == inside)
                .count();
        let title = RichText::new(format!("{} ({count})", bin.name)).strong();
        let shown = CollapsingHeader::new(title)
            .id_salt(("bin", bin.id.0))
            .show(ui, |ui| self.contents(ui, inside));
        let header = shown.header_response.interact(Sense::click_and_drag());
        if header.drag_started() {
            header.dnd_set_drag_payload(bin.id);
        }
        self.drop_target(ui, &header, inside);
        header.context_menu(|ui| {
            if ui.button("New Bin Inside").clicked() {
                self.action = Some(PoolAction::NewBin(inside));
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
        let offline = self.offline.contains(&item.id);
        let response = item_row(ui, item, self.sequence.timebase, building, offline);
        if response.drag_started() {
            response.dnd_set_drag_payload(item.id);
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
        // In project order, whatever order they were picked in.
        let cameras: Vec<SourceId> = self
            .sequence
            .media
            .iter()
            .filter(|item| selected.contains(&item.id) && item.is_file() && item.video.is_some())
            .map(|item| item.id)
            .collect();
        if response.double_clicked() && item.nest.is_some() {
            self.action = Some(PoolAction::OpenNest(item.id));
        }
        response.context_menu(|ui| {
            if ui.button("Open in Source Monitor").clicked() {
                self.action = Some(PoolAction::Open(item.id));
            }
            let can_sync = cameras.len() >= 2 && cameras.contains(&item.id);
            ui.add_enabled_ui(can_sync, |ui| {
                ui.menu_button("Create Multi-Camera Source Sequence", |ui| {
                    for (sync, label) in [
                        (MulticamSync::Start, "Synchronize by Start"),
                        (MulticamSync::Audio, "Synchronize by Audio"),
                    ] {
                        if ui.button(label).clicked() {
                            self.action = Some(PoolAction::CreateMulticam(cameras.clone(), sync));
                        }
                    }
                })
            })
            .response
            .on_disabled_hover_text("Ctrl+click two or more video clips to use as cameras");
            if item.nest.is_some() && ui.button("Open in Timeline").clicked() {
                self.action = Some(PoolAction::OpenNest(item.id));
            }
            if ui.button("Rename").clicked() {
                self.view.renaming = Some((entry, item.name.clone()));
            }
            if item.bin.is_some() && ui.button("Move to Top").clicked() {
                self.action = Some(PoolAction::Move(entry, None));
            }
            let relink = if offline {
                "Link Media…"
            } else {
                "Replace Footage…"
            };
            if item.is_file() && ui.button(relink).clicked() {
                self.action = Some(PoolAction::Replace(item.id));
            }
            if ui.button("Remove from Project").clicked() {
                self.action = Some(PoolAction::Remove(item.id));
            }
            if item.video.is_some() && item.is_file() {
                if ui.button("Interpret Footage…").clicked() {
                    self.action = Some(PoolAction::Interpret(item.id));
                }
                ui.separator();
                let can_build = self.proxies.is_some() && building.is_none();
                proxy_menu(ui, item, can_build, &mut self.action);
            }
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

    /// The space below the list, where a drop moves things out of every bin.
    fn top_drop_zone(&mut self, ui: &mut egui::Ui) {
        let size = egui::vec2(ui.available_width(), 32.0);
        let (rect, zone) = ui.allocate_exact_size(size, Sense::click());
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
