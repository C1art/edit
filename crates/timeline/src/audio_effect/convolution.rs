use serde::{Deserialize, Serialize};

/// The space whose response the Convolution Reverb plays a sound through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum ReverbSpace {
    SmallRoom,
    #[default]
    MediumRoom,
    ConcertHall,
    Cathedral,
    Plate,
}

impl ReverbSpace {
    pub const ALL: [Self; 5] = [
        Self::SmallRoom,
        Self::MediumRoom,
        Self::ConcertHall,
        Self::Cathedral,
        Self::Plate,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::SmallRoom => "Small Room",
            Self::MediumRoom => "Medium Room",
            Self::ConcertHall => "Concert Hall",
            Self::Cathedral => "Cathedral",
            Self::Plate => "Plate",
        }
    }
}
