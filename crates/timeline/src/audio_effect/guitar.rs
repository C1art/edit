use serde::{Deserialize, Serialize};

/// The amplifier and speaker cabinet the Guitar Suite plays through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum AmpModel {
    None,
    Small,
    #[default]
    Combo,
    Stack,
    Bass,
}

impl AmpModel {
    pub const ALL: [Self; 5] = [
        Self::None,
        Self::Small,
        Self::Combo,
        Self::Stack,
        Self::Bass,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Small => "Small Practice Amp",
            Self::Combo => "Combo Amp",
            Self::Stack => "British Stack",
            Self::Bass => "Bass Amp",
        }
    }
}

/// The resonant filter that ends the Guitar Suite's chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum GuitarFilter {
    #[default]
    None,
    LowPass,
    BandPass,
    HighPass,
}

impl GuitarFilter {
    pub const ALL: [Self; 4] = [Self::None, Self::LowPass, Self::BandPass, Self::HighPass];

    pub fn name(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::LowPass => "Low Pass",
            Self::BandPass => "Band Pass",
            Self::HighPass => "High Pass",
        }
    }
}
