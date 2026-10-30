use crate::edit::ops;
use crate::{
    ClipId, Edit, Marker, MarkerColor, MarkerId, MarkerKind, Sequence, Ticks, TimelineError,
};

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
            duration: Ticks::ZERO,
            name: String::new(),
            comment: String::new(),
            color: MarkerColor::default(),
            kind: MarkerKind::default(),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClearMarkers {
    pub owner: MarkerOwner,
}

impl Edit for ClearMarkers {
    fn name(&self) -> &'static str {
        "Clear All Markers"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let markers = markers_mut(sequence, self.owner)?;
        if markers.is_empty() {
            return Err(TimelineError::NothingToEdit);
        }
        markers.clear();
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
        if self.marker.duration < Ticks::ZERO {
            return Err(TimelineError::NothingToEdit);
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
