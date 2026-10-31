use crate::{Edit, Guide, Sequence, TimelineError};

/// Adds, moves or removes the sequence's guides, by index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuideEdit {
    Add(Guide),
    Move { index: usize, to: i32 },
    Remove(usize),
    Clear,
}

impl Edit for GuideEdit {
    fn name(&self) -> &'static str {
        match self {
            Self::Add(_) => "Add Guide",
            Self::Move { .. } => "Move Guide",
            Self::Remove(_) => "Remove Guide",
            Self::Clear => "Clear Guides",
        }
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let guides = &mut sequence.guides;
        match *self {
            Self::Add(guide) if !guides.contains(&guide) => guides.push(guide),
            Self::Move { index, to } => {
                let guide = guides.get_mut(index).ok_or(TimelineError::NothingToEdit)?;
                if guide.at == to {
                    return Err(TimelineError::NothingToEdit);
                }
                guide.at = to;
            }
            Self::Remove(index) if index < guides.len() => {
                guides.remove(index);
            }
            Self::Clear if !guides.is_empty() => guides.clear(),
            _ => return Err(TimelineError::NothingToEdit),
        }
        Ok(())
    }
}
