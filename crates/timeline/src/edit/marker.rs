use crate::{Edit, Marker, MarkerColor, MarkerId, Sequence, Ticks, TimelineError};

/// Adds an unnamed marker, unless one is already at `at`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddMarker {
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
        if sequence.markers.iter().any(|marker| marker.at == self.at) {
            return Err(TimelineError::NothingToEdit);
        }
        let marker = Marker {
            id: MarkerId(sequence.ids.next()),
            at: self.at,
            name: String::new(),
            comment: String::new(),
            color: MarkerColor::default(),
        };
        let index = sequence.markers.partition_point(|m| m.at < self.at);
        sequence.markers.insert(index, marker);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveMarker {
    pub id: MarkerId,
}

impl Edit for RemoveMarker {
    fn name(&self) -> &'static str {
        "Clear Marker"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let count = sequence.markers.len();
        sequence.markers.retain(|marker| marker.id != self.id);
        if sequence.markers.len() == count {
            return Err(TimelineError::MarkerNotFound(self.id));
        }
        Ok(())
    }
}

/// Replaces a marker's position, name, comment and color, keeping markers in time order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateMarker {
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
        let index = sequence
            .markers
            .iter()
            .position(|marker| marker.id == self.marker.id)
            .ok_or(TimelineError::MarkerNotFound(self.marker.id))?;
        if sequence.markers[index] == self.marker {
            return Err(TimelineError::NothingToEdit);
        }
        sequence.markers.remove(index);
        let index = sequence.markers.partition_point(|m| m.at <= self.marker.at);
        sequence.markers.insert(index, self.marker.clone());
        Ok(())
    }
}
