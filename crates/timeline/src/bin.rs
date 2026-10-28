use serde::{Deserialize, Serialize};

use crate::Sequence;

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
}

impl Sequence {
    pub fn bin(&self, id: BinId) -> Option<&Bin> {
        self.bins.iter().find(|bin| bin.id == id)
    }

    /// Whether `bin` is `ancestor` or lies anywhere inside it.
    pub fn bin_within(&self, bin: BinId, ancestor: BinId) -> bool {
        // Bounded so a damaged project file whose bins hold each other can't hang.
        std::iter::successors(Some(bin), |&id| self.bin(id)?.parent)
            .take(self.bins.len() + 1)
            .any(|id| id == ancestor)
    }
}
