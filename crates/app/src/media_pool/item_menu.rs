use std::iter;

use timeline::{MediaItem, Sequence, SourceId};

use super::rows::proxy_menu;
use super::{Pool, PoolAction, PoolEntry};
use crate::multicam_sync::MulticamSync;

impl Pool<'_> {
    pub(super) fn item_menu(
        &mut self,
        ui: &mut egui::Ui,
        item: &MediaItem,
        offline: bool,
        building: Option<f32>,
    ) {
        let entry = PoolEntry::Media(item.id);
        // In project order, whatever order they were picked in.
        let cameras: Vec<SourceId> = self
            .sequence
            .media
            .iter()
            .filter(|item| {
                self.view.selected.contains(&item.id) && item.is_file() && item.video.is_some()
            })
            .map(|item| item.id)
            .collect();
        let merge = merge_sources(self.sequence, &self.view.selected, item.id);
        let to_take_offline: Vec<SourceId> = if self.view.selected.contains(&item.id) {
            self.sequence
                .media
                .iter()
                .filter(|item| {
                    self.view.selected.contains(&item.id) && item.is_file() && !item.offline
                })
                .map(|item| item.id)
                .collect()
        } else {
            vec![item.id]
        };
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
        ui.add_enabled_ui(merge.is_some(), |ui| {
            ui.menu_button("Merge Clips", |ui| {
                ui.checkbox(
                    &mut self.view.remove_camera_audio,
                    "Remove Audio from AV Clip",
                );
                for (sync, label) in [
                    (MulticamSync::Start, "Synchronize by Start"),
                    (MulticamSync::Audio, "Synchronize by Audio"),
                ] {
                    if let Some(sources) = merge.as_ref().filter(|_| ui.button(label).clicked()) {
                        let remove = self.view.remove_camera_audio;
                        self.action = Some(PoolAction::Merge(sources.clone(), sync, remove));
                    }
                }
            })
        })
        .response
        .on_disabled_hover_text("Ctrl+click one video clip and the audio clips recorded with it");
        let playable = item.video.is_some() || item.has_audio;
        if playable && ui.button("Automate to Sequence…").clicked() {
            if !self.view.selected.contains(&item.id) {
                self.view.selected = vec![item.id];
            }
            self.action = Some(PoolAction::Automate);
        }
        if playable && ui.button("New Sequence From Clip").clicked() {
            self.action = Some(PoolAction::NewSequenceFrom(item.id));
        }
        if item.nest.is_some() && ui.button("Open in Timeline").clicked() {
            self.action = Some(PoolAction::OpenNest(item.id));
        }
        if item.subclip.is_some() && ui.button("Edit Subclip…").clicked() {
            self.action = Some(PoolAction::EditSubclip(item.id));
        }
        if ui.button("Rename").clicked() {
            self.view.renaming = Some((entry, item.name.clone()));
        }
        if ui.button("Duplicate").clicked() {
            self.action = Some(PoolAction::Duplicate(item.id));
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
        if item.is_file() && !item.offline && ui.button("Reveal in Explorer").clicked() {
            self.action = Some(PoolAction::RevealInExplorer(item.id));
        }
        if item.is_file() && !item.offline && ui.button("Edit Original").clicked() {
            self.action = Some(PoolAction::EditOriginal(item.id));
        }
        if item.is_file() && !item.offline && ui.button("Make Offline").clicked() {
            self.action = Some(PoolAction::MakeOffline(to_take_offline));
        }
        if ui.button("Remove from Project").clicked() {
            self.action = Some(PoolAction::Remove(item.id));
        }
        if item.has_audio && item.is_file() && ui.button("Modify Audio Channels…").clicked() {
            self.action = Some(PoolAction::AudioChannels(item.id));
        }
        if item.has_audio && item.is_file() && ui.button("Breakout to Mono").clicked() {
            self.action = Some(PoolAction::BreakoutToMono(item.id));
        }
        if item.video.is_some() && item.is_file() {
            if ui.button("Interpret Footage…").clicked() {
                self.action = Some(PoolAction::Interpret(item.id));
            }
            if ui.button("Modify Timecode…").clicked() {
                self.action = Some(PoolAction::Timecode(item.id));
            }
            ui.separator();
            let can_build = self.proxies.is_some() && building.is_none();
            proxy_menu(ui, item, can_build, &mut self.action);
        }
    }
}

/// The selected picture item first and then the selected sound-only items, when the
/// selection holding `item` is one of each or more sound.
fn merge_sources(
    sequence: &Sequence,
    selected: &[SourceId],
    item: SourceId,
) -> Option<Vec<SourceId>> {
    if !selected.contains(&item) {
        return None;
    }
    let (pictures, sounds): (Vec<&MediaItem>, Vec<&MediaItem>) = sequence
        .media
        .iter()
        .filter(|item| selected.contains(&item.id) && item.is_file())
        .partition(|item| item.video.is_some());
    let all_sound = sounds.iter().all(|item| item.has_audio);
    let [picture] = pictures[..] else {
        return None;
    };
    (!sounds.is_empty() && all_sound).then(|| {
        iter::once(picture)
            .chain(sounds)
            .map(|item| item.id)
            .collect()
    })
}
