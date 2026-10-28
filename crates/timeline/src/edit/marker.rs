use crate::edit::ops;
use crate::{ClipId, Edit, Marker, MarkerColor, MarkerId, Sequence, Ticks, TimelineError};

/// Whose markers an edit changes. A clip's markers are timed in its source.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MarkerOwner {
    #[default]
    Sequence,
    Clip(ClipId),
}

fn markers_mut(
    sequence: &mut Sequence,
    owner: MarkerOwner,
) -> Result<&mut Vec<Marker>, TimelineError> {
    match owner {
        MarkerOwner::Sequence => Ok(&mut sequence.markers),
        MarkerOwner::Clip(id) => Ok(&mut ops::clip_mut(sequence, id)?.markers),
    }
}

/// Adds an unnamed marker, unless one is already at `at`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddMarker {
    pub owner: MarkerOwner,
    pub at: Ticks,
}

impl Edit for AddMarker {
    fn name(&self) -> &'static str {
        "Add Marker"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        if self.at < Ticks::ZERO {
            return Err(TimelineError::BeforeSequenceStart);
        }
        let id = MarkerId(sequence.ids.next());
        let markers = markers_mut(sequence, self.owner)?;
        if markers.iter().any(|marker| marker.at == self.at) {
            return Err(TimelineError::NothingToEdit);
        }
        let marker = Marker {
            id,
            at: self.at,
            name: String::new(),
            comment: String::new(),
            color: MarkerColor::default(),
        };
        let index = markers.partition_point(|m| m.at < self.at);
        markers.insert(index, marker);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveMarker {
    pub owner: MarkerOwner,
    pub id: MarkerId,
}

impl Edit for RemoveMarker {
    fn name(&self) -> &'static str {
        "Clear Marker"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let markers = markers_mut(sequence, self.owner)?;
        let count = markers.len();
        markers.retain(|marker| marker.id != self.id);
        if markers.len() == count {
            return Err(TimelineError::MarkerNotFound(self.id));
        }
        Ok(())
    }
}

/// Replaces a marker's position, name, comment and color, keeping markers in time order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateMarker {
    pub owner: MarkerOwner,
    pub marker: Marker,
}

impl Edit for UpdateMarker {
    fn name(&self) -> &'static str {
        "Edit Marker"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        if self.marker.at < Ticks::ZERO {
            return Err(TimelineError::BeforeSequenceStart);
        }
        let markers = markers_mut(sequence, self.owner)?;
        let index = markers
            .iter()
            .position(|marker| marker.id == self.marker.id)
            .ok_or(TimelineError::MarkerNotFound(self.marker.id))?;
        if markers[index] == self.marker {
            return Err(TimelineError::NothingToEdit);
        }
        markers.remove(index);
        let index = markers.partition_point(|m| m.at <= self.marker.at);
        markers.insert(index, self.marker.clone());
        Ok(())
    }
}
