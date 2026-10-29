use crate::xml::Element;
use crate::{FrameRate, IdGenerator, Marker, MarkerId, Ticks, Timebase};

/// A marker's frames, `out` -1 for one at a single frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct XmlMarker {
    name: String,
    comment: String,
    at: i64,
    out: i64,
}

impl XmlMarker {
    fn read(element: &Element) -> Option<Self> {
        let text = |tag| element.text_at(&[tag]).unwrap_or_default().to_owned();
        Some(Self {
            name: text("name"),
            comment: text("comment"),
            at: element.number_at(&["in"]).filter(|&at| at >= 0)?,
            out: element.number_at(&["out"]).unwrap_or(-1),
        })
    }

    pub(crate) fn all(element: &Element) -> Vec<Self> {
        element.children("marker").filter_map(Self::read).collect()
    }

    pub(crate) fn marker(
        &self,
        ids: &mut IdGenerator,
        rate: FrameRate,
        timebase: Timebase,
    ) -> Marker {
        let at = rate.frame_start(self.at, timebase);
        let end = rate.frame_start(self.out, timebase);
        Marker {
            id: MarkerId(ids.next()),
            at,
            duration: if self.out > self.at {
                end - at
            } else {
                Ticks::ZERO
            },
            name: self.name.clone(),
            comment: self.comment.clone(),
            color: Default::default(),
            kind: Default::default(),
        }
    }
}
