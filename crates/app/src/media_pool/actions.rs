use std::path::PathBuf;

use timeline::{
    AddBin, Edit, MoveToBin, RelinkMedia, RemoveBin, RemoveMedia, RenameBin, RenameMedia, SetProxy,
    SourceId, Ticks, TimelineError,
};

use super::{PoolAction, PoolEntry};
use crate::editor::Editor;
use crate::import::Probed;
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
