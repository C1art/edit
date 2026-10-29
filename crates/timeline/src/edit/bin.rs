use crate::{Bin, BinId, Edit, Sequence, SourceId, TimelineError};

fn check_exists(sequence: &Sequence, bin: Option<BinId>) -> Result<(), TimelineError> {
    match bin {
        Some(id) if sequence.bin(id).is_none() => Err(TimelineError::BinNotFound(id)),
        _ => Ok(()),
    }
}

/// Adds an empty bin inside `parent`, or at the top of the project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddBin {
    pub name: String,
    pub parent: Option<BinId>,
}

impl Edit for AddBin {
    fn name(&self) -> &'static str {
        "New Bin"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        check_exists(sequence, self.parent)?;
        let name = match self.name.trim() {
            "" => "Bin",
            name => name,
        };
        let id = BinId(sequence.ids.next());
        sequence.bins.push(Bin {
            id,
            name: name.to_owned(),
            parent: self.parent,
        });
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameBin {
    pub bin: BinId,
    pub name: String,
}

impl Edit for RenameBin {
    fn name(&self) -> &'static str {
        "Rename Bin"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let name = self.name.trim();
        let bin = sequence
            .bins
            .iter_mut()
            .find(|bin| bin.id == self.bin)
            .ok_or(TimelineError::BinNotFound(self.bin))?;
        if name.is_empty() || bin.name == name {
            return Err(TimelineError::NothingToEdit);
        }
        name.clone_into(&mut bin.name);
        Ok(())
    }
}

/// Deletes a bin, handing what it held to the bin around it so no media leaves the
/// project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveBin {
    pub bin: BinId,
}

impl Edit for RemoveBin {
    fn name(&self) -> &'static str {
        "Delete Bin"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let parent = sequence
            .bin(self.bin)
            .ok_or(TimelineError::BinNotFound(self.bin))?
            .parent;
        let removed = Some(self.bin);
        for item in sequence.media.iter_mut().filter(|item| item.bin == removed) {
            item.bin = parent;
        }
        for bin in sequence.bins.iter_mut().filter(|bin| bin.parent == removed) {
            bin.parent = parent;
        }
        sequence.bins.retain(|bin| bin.id != self.bin);
        Ok(())
    }
}

/// Moves media items and bins into bin `to`, or to the top of the project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveToBin {
    pub media: Vec<SourceId>,
    pub bins: Vec<BinId>,
    pub to: Option<BinId>,
}

impl Edit for MoveToBin {
    fn name(&self) -> &'static str {
        "Move to Bin"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        check_exists(sequence, self.to)?;
        for &bin in &self.bins {
            check_exists(sequence, Some(bin))?;
            if self.to.is_some_and(|to| sequence.bin_within(to, bin)) {
                return Err(TimelineError::BinCycle(bin));
            }
        }
        if let Some(&missing) = self
            .media
            .iter()
            .find(|&&id| sequence.media_item(id).is_none())
        {
            return Err(TimelineError::MediaNotFound(missing));
        }
        let mut moved = false;
        for item in &mut sequence.media {
            if self.media.contains(&item.id) && item.bin != self.to {
                item.bin = self.to;
                moved = true;
            }
        }
        for bin in &mut sequence.bins {
            if self.bins.contains(&bin.id) && bin.parent != self.to {
                bin.parent = self.to;
                moved = true;
            }
        }
        if moved {
            Ok(())
        } else {
            Err(TimelineError::NothingToEdit)
        }
    }
}
