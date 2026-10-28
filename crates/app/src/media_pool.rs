use std::path::PathBuf;

use egui::{Key, Label, RichText, Sense, TextEdit};
use timeline::{
    Edit, FrameRate, MediaItem, RelinkMedia, RemoveMedia, RenameMedia, SetProxy, SourceId, Ticks,
    Timebase, Timecode, TimelineError,
};

use crate::editor::Editor;
use crate::import::Probed;
use crate::project_files::PathPurpose;
use crate::proxies::ProxyMaker;

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
}

/// The search text, and the item being renamed with its name so far.
type PoolState = (String, Option<(SourceId, String)>);

enum Field {
    Editing,
    Done,
    Cancelled,
}

/// Lists imported media. Click opens an item in the source monitor; dragging one onto
/// the timeline places it.
pub fn ui(
    ui: &mut egui::Ui,
    media: &[MediaItem],
    timebase: Timebase,
    importing: bool,
    proxies: Option<&ProxyMaker>,
) -> Option<PoolAction> {
    let mut action = None;
    if media.is_empty() && !importing {
        ui.label("Drop files here or use File → Import (Ctrl+I).");
    }
    let state_id = ui.scope_id().with("pool");
    let (mut search, mut renaming): PoolState =
        ui.data_mut(|data| data.get_temp(state_id).unwrap_or_default());
    if !media.is_empty() {
        ui.add(
            TextEdit::singleline(&mut search)
                .hint_text("Search")
                .desired_width(f32::INFINITY),
        );
    }
    let wanted = search.to_lowercase();
    egui::ScrollArea::vertical()
        .auto_shrink([false, true])
        .max_height(ui.available_height() * 0.6)
        .show(ui, |ui| {
            for item in media
                .iter()
                .filter(|item| item.name.to_lowercase().contains(&wanted))
            {
                if let Some((id, name)) = &mut renaming {
                    if *id == item.id {
                        match rename_field(ui, name) {
                            Field::Editing => {}
                            Field::Done => {
                                action = Some(PoolAction::Rename(*id, std::mem::take(name)));
                                renaming = None;
                            }
                            Field::Cancelled => renaming = None,
                        }
                        continue;
                    }
                }
                let building = proxies.and_then(|maker| maker.progress(item.id));
                let response = item_row(ui, item, timebase, building);
                if response.drag_started() {
                    response.dnd_set_drag_payload(item.id);
                }
                if response.clicked() {
                    action = Some(PoolAction::Open(item.id));
                }
                if response.double_clicked() && item.nest.is_some() {
                    action = Some(PoolAction::OpenNest(item.id));
                }
                response.context_menu(|ui| {
                    if ui.button("Open in Source Monitor").clicked() {
                        action = Some(PoolAction::Open(item.id));
                    }
                    if item.nest.is_some() && ui.button("Open in Timeline").clicked() {
                        action = Some(PoolAction::OpenNest(item.id));
                    }
                    if ui.button("Rename").clicked() {
                        renaming = Some((item.id, item.name.clone()));
                    }
                    if item.is_file() && ui.button("Replace Footage…").clicked() {
                        action = Some(PoolAction::Replace(item.id));
                    }
                    if ui.button("Remove from Project").clicked() {
                        action = Some(PoolAction::Remove(item.id));
                    }
                    if item.video.is_some() && item.is_file() {
                        ui.separator();
                        proxy_menu(
                            ui,
                            item,
                            proxies.is_some() && building.is_none(),
                            &mut action,
                        );
                    }
                });
            }
        });
    ui.data_mut(|data| data.insert_temp(state_id, (search, renaming)));
    if importing {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Importing…");
        });
    }
    action
}

fn proxy_menu(
    ui: &mut egui::Ui,
    item: &MediaItem,
    can_build: bool,
    action: &mut Option<PoolAction>,
) {
    let label = if item.proxy.is_some() {
        "Recreate Proxy"
    } else {
        "Create Proxy"
    };
    if ui
        .add_enabled(can_build, egui::Button::new(label))
        .clicked()
    {
        *action = Some(PoolAction::CreateProxy(item.id));
    }
    let detach = egui::Button::new("Detach Proxy");
    if ui.add_enabled(item.proxy.is_some(), detach).clicked() {
        *action = Some(PoolAction::DetachProxy(item.id));
    }
}

/// A field taking an item's new name: Enter or clicking away keeps it, Escape drops it.
fn rename_field(ui: &mut egui::Ui, name: &mut String) -> Field {
    let response = ui.add(TextEdit::singleline(name).desired_width(f32::INFINITY));
    if response.lost_focus() {
        if ui.input(|input| input.key_pressed(Key::Escape)) {
            Field::Cancelled
        } else {
            Field::Done
        }
    } else {
        if !response.has_focus() {
            response.request_focus();
        }
        Field::Editing
    }
}

fn item_row(
    ui: &mut egui::Ui,
    item: &MediaItem,
    timebase: Timebase,
    building: Option<f32>,
) -> egui::Response {
    let rate = item
        .video
        .map_or(FrameRate::FPS_25, |video| video.frame_rate);
    let mut details = vec![match &item.generator {
        Some(generator) => generator.kind_name().to_owned(),
        None => Timecode::new(item.duration, timebase, rate).to_string(),
    }];
    if item.nest.is_some() {
        details.insert(0, "Sequence".to_owned());
    }
    if let Some(video) = item.video {
        details.push(format!(
            "{}×{} {:.2} fps",
            video.width,
            video.height,
            video.frame_rate.approx_fps()
        ));
    }
    if item.has_audio {
        details.push("audio".to_owned());
    }
    if item.proxy.is_some() {
        details.push("proxy".to_owned());
    }
    let inner = ui.group(|ui| {
        ui.set_width(ui.available_width());
        ui.add(Label::new(RichText::new(&item.name).strong()).truncate());
        ui.add(Label::new(RichText::new(details.join(" · ")).small()).truncate());
        if let Some(done) = building {
            let bar = egui::ProgressBar::new(done).text(RichText::new("Creating proxy").small());
            ui.add(bar.desired_height(14.0));
        }
    });
    inner
        .response
        .interact(Sense::click_and_drag())
        .on_hover_text(match (&item.generator, &item.nest) {
            (Some(generator), _) => generator.kind_name().to_owned(),
            (None, Some(_)) => "Nested sequence; double-click to open".to_owned(),
            (None, None) => item.path.display().to_string(),
        })
}

impl Editor {
    pub(crate) fn pool_action(&mut self, action: PoolAction) {
        match action {
            PoolAction::Open(id) => self.open_source(id),
            PoolAction::OpenNest(id) => self.enter_nest(id, Ticks::ZERO),
            PoolAction::Remove(id) => {
                if self.source_shows(id) {
                    self.source = None;
                }
                self.apply_to_pool(RemoveMedia { source: id });
            }
            PoolAction::Rename(source, name) => {
                self.apply_to_pool(RenameMedia { source, name });
            }
            PoolAction::Replace(id) => self.prompt_path(PathPurpose::Replace(id)),
            PoolAction::CreateProxy(id) => {
                let sequence = self.timeline.sequence();
                let path = sequence.media_item(id).map(|item| item.path.clone());
                if let (Some(maker), Some(path)) = (&mut self.proxies, path) {
                    maker.request(id, path);
                }
            }
            PoolAction::DetachProxy(id) => {
                self.apply(SetProxy {
                    source: id,
                    proxy: None,
                });
            }
        }
    }

    /// Probes `path` to add to the pool, or to replace the file of `replacing`.
    pub(crate) fn import(&mut self, path: PathBuf, replacing: Option<SourceId>) {
        match &mut self.importer {
            Some(importer) => importer.request(path, replacing),
            None => self.notice(format!("Cannot import {}", path.display())),
        }
    }

    pub(crate) fn poll_imports(&mut self) {
        let Some(importer) = &mut self.importer else {
            return;
        };
        for Probed { replacing, result } in importer.poll() {
            match (result, replacing) {
                (Ok(media), Some(source)) => {
                    if self.apply_to_pool(RelinkMedia { source, media })
                        && self.source_shows(source)
                    {
                        self.open_source(source);
                    }
                }
                (Ok(import), None) => {
                    if self.apply(import) {
                        if let Some(id) = self.timeline.sequence().media.last().map(|item| item.id)
                        {
                            self.open_source(id);
                        }
                    }
                }
                (Err((path, error)), _) => {
                    self.notice(format!("Cannot use {}: {error}", path.display()));
                }
            }
        }
    }

    fn source_shows(&self, id: SourceId) -> bool {
        self.source
            .as_ref()
            .is_some_and(|source| source.source == id)
    }

    /// Applies an edit to the media pool, which belongs to the project even inside a nest.
    fn apply_to_pool(&mut self, edit: impl Edit) -> bool {
        match self.timeline.apply_to_project(edit) {
            Ok(()) => true,
            Err(TimelineError::NothingToEdit) => false,
            Err(error) => {
                self.notice(format!("Cannot change media: {error}"));
                false
            }
        }
    }
}
