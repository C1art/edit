use serde::{Deserialize, Serialize};

use crate::{MediaItem, Sequence};

/// Identifies a bin in the project's media pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BinId(pub u64);

/// A folder of media items and other bins in the project panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bin {
    pub id: BinId,
    pub name: String,
    /// The bin holding this one, or none at the top of the project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<BinId>,
    /// Makes this a search bin, showing every item that matches instead of its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search: Option<BinSearch>,
}

/// What a search bin finds: each item whose `field` holds `text`, ignoring case.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BinSearch {
    pub text: String,
    #[serde(default)]
    pub field: SearchField,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchField {
    #[default]
    All,
    Name,
    FilePath,
    Description,
    Scene,
    Shot,
    LogNote,
}

impl SearchField {
    pub const ALL: [Self; 7] = [
        Self::All,
        Self::Name,
        Self::FilePath,
        Self::Description,
        Self::Scene,
        Self::Shot,
        Self::LogNote,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Name => "Name",
            Self::FilePath => "File Path",
            Self::Description => "Description",
            Self::Scene => "Scene",
            Self::Shot => "Shot",
            Self::LogNote => "Log Note",
        }
    }
}

impl BinSearch {
    pub fn matches(&self, item: &MediaItem) -> bool {
        let wanted = self.text.trim().to_lowercase();
        if wanted.is_empty() {
            return false;
        }
        let path = item.path.to_string_lossy();
        let log = &item.log;
        let holds = |text: &str| text.to_lowercase().contains(&wanted);
        match self.field {
            SearchField::All => [
                item.name.as_str(),
                &path,
                &log.description,
                &log.scene,
                &log.shot,
                &log.log_note,
            ]
            .into_iter()
            .any(holds),
            SearchField::Name => holds(&item.name),
            SearchField::FilePath => holds(&path),
            SearchField::Description => holds(&log.description),
            SearchField::Scene => holds(&log.scene),
            SearchField::Shot => holds(&log.shot),
            SearchField::LogNote => holds(&log.log_note),
        }
    }
}

impl Sequence {
    pub fn bin(&self, id: BinId) -> Option<&Bin> {
        self.bins.iter().find(|bin| bin.id == id)
    }

    /// What bin `id` shows: its search's matches for a search bin, otherwise the items
    /// filed in it.
    pub fn bin_items(&self, id: Option<BinId>) -> impl Iterator<Item = &MediaItem> {
        let search = id.and_then(|id| self.bin(id)?.search.as_ref());
        self.media.iter().filter(move |item| match search {
            Some(search) => search.matches(item),
            None => item.bin == id,
        })
    }

    /// Whether `bin` is `ancestor` or lies anywhere inside it.
    pub fn bin_within(&self, bin: BinId, ancestor: BinId) -> bool {
        // Bounded so a damaged project file whose bins hold each other can't hang.
        std::iter::successors(Some(bin), |&id| self.bin(id)?.parent)
            .take(self.bins.len() + 1)
            .any(|id| id == ancestor)
    }
}
