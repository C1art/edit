use std::path::{Path, PathBuf};

use timeline::{
    AddBin, CreateMulticam, DuplicateMedia, Edit, InterpretFootage, MakeOffline, MakeSubclip,
    MergeClips, MoveToBin, RelinkMedia, RemoveBin, RemoveMedia, RenameBin, RenameMedia, SetProxy,
    SourceId, Ticks, TimelineError,
};

use super::{PoolAction, PoolEntry};
use crate::editor::Editor;
use crate::import::Probed;
use crate::multicam_sync::Synced;
use crate::project_files::PathPurpose;

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
            PoolAction::Duplicate(source) => {
                self.apply_to_pool(DuplicateMedia { source });
            }
            PoolAction::Rename(source, name) => {
                self.apply_to_pool(RenameMedia { source, name });
            }
            PoolAction::Replace(id) => self.prompt_path(PathPurpose::Replace(id)),
            PoolAction::MakeOffline(sources) => {
                self.apply_to_pool(MakeOffline { sources });
            }
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
            PoolAction::Interpret(id) => self.open_interpret_footage(id),
            PoolAction::AudioChannels(id) => self.open_audio_channels(id),
            PoolAction::NewBin(parent) => self.new_bin(parent),
            PoolAction::RenameBin(bin, name) => {
                self.apply_to_pool(RenameBin { bin, name });
            }
            PoolAction::RemoveBin(bin) => {
                self.apply_to_pool(RemoveBin { bin });
            }
            PoolAction::Move(entry, to) => {
                let (media, bins) = match entry {
                    PoolEntry::Media(id) => (vec![id], Vec::new()),
                    PoolEntry::Bin(id) => (Vec::new(), vec![id]),
                };
                self.apply_to_pool(MoveToBin { media, bins, to });
            }
            PoolAction::CreateMulticam(sources, sync) => {
                self.sync_sources(sources, sync, Synced::Multicam)
            }
            PoolAction::Merge(sources, sync, remove_camera_audio) => {
                let synced = Synced::Merge {
                    remove_camera_audio,
                };
                self.sync_sources(sources, sync, synced)
            }
            PoolAction::NewSequenceFrom(source) => self.new_sequence_from(source),
        }
    }

    /// Probes `path` to add to the pool, or to replace the file of `replacing`.
    pub(crate) fn import(&mut self, path: PathBuf, replacing: Option<SourceId>) {
        match &mut self.importer {
            Some(importer) => {
                let rate = self.timeline.sequence().format.frame_rate;
                importer.request(path, replacing, rate);
            }
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
                    let folder = media.path.parent().map(Path::to_owned);
                    if self.apply_to_pool(RelinkMedia { source, media }) {
                        self.relinked(source, folder);
                        if self.source_shows(source) {
                            self.open_source(source);
                        }
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

    /// Adds a bin numbered after the project's others and opens its name for typing.
    fn new_bin(&mut self, parent: Option<timeline::BinId>) {
        let name = format!("Bin {}", self.timeline.project().bins.len() + 1);
        if self.apply_to_pool(AddBin {
            name: name.clone(),
            parent,
        }) {
            if let Some(bin) = self.timeline.project().bins.last() {
                self.pool.renaming = Some((PoolEntry::Bin(bin.id), name));
            }
        }
    }

    pub(crate) fn create_multicam(&mut self, sources: Vec<SourceId>, starts: Vec<Ticks>) {
        let pool = self.timeline.project();
        let first = sources.first().and_then(|&id| pool.media_item(id));
        let name = first.map_or_else(
            || "Multicam".to_owned(),
            |item| format!("{} Multicam", item.name),
        );
        if self.apply_to_pool(CreateMulticam {
            sources,
            starts,
            name,
        }) {
            if let Some(item) = self.timeline.project().media.last() {
                self.pool.selected = vec![item.id];
                self.notice(format!(
                    "Created {}; place it in a sequence and turn on Multi-Camera in the program monitor's View menu",
                    item.name
                ));
            }
        }
    }

    pub(crate) fn create_merged(
        &mut self,
        sources: Vec<SourceId>,
        starts: Vec<Ticks>,
        remove_camera_audio: bool,
    ) {
        let pool = self.timeline.project();
        let camera = sources
            .iter()
            .filter_map(|&id| pool.media_item(id))
            .find(|item| item.video.is_some());
        let name = camera.map_or_else(
            || "Merged".to_owned(),
            |item| format!("{} Merged", item.name),
        );
        if self.apply_to_pool(MergeClips {
            sources,
            starts,
            remove_camera_audio,
            name,
        }) {
            if let Some(item) = self.timeline.project().media.last() {
                self.pool.selected = vec![item.id];
            }
        }
    }

    /// Adds the source monitor's marked range to the project as a subclip.
    pub(crate) fn make_subclip(&mut self) {
        let Some(source) = &self.source else {
            self.notice("Open a clip in the source monitor and mark the part to keep first");
            return;
        };
        let (start, duration) = source.range();
        let edit = MakeSubclip {
            source: source.source,
            range: start..start + duration,
            name: format!("{}.Subclip", source.name),
        };
        if self.apply_to_pool(edit) {
            if let Some(item) = self.timeline.project().media.last() {
                self.pool.selected = vec![item.id];
            }
        }
    }

    fn source_shows(&self, id: SourceId) -> bool {
        self.source
            .as_ref()
            .is_some_and(|source| source.source == id)
    }

    /// Applies an edit to the media pool, which belongs to the project even inside a nest.
    pub(crate) fn interpret_footage(&mut self, edit: InterpretFootage) {
        self.apply_to_pool(edit);
    }

    pub(crate) fn apply_to_pool(&mut self, edit: impl Edit) -> bool {
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
