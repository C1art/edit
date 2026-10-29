use serde::{Deserialize, Serialize};

/// The Scientific Filter's design, which trades slope against ripple and ringing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum FilterType {
    Bessel,
    #[default]
    Butterworth,
    Chebyshev,
}

impl FilterType {
    pub const ALL: [Self; 3] = [Self::Bessel, Self::Butterworth, Self::Chebyshev];

    pub fn name(self) -> &'static str {
        match self {
            Self::Bessel => "Bessel",
            Self::Butterworth => "Butterworth",
            Self::Chebyshev => "Chebyshev",
        }
    }
}

/// Which frequencies the Scientific Filter lets through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum FilterMode {
    #[default]
    LowPass,
    HighPass,
    BandPass,
    BandStop,
}

impl FilterMode {
    pub const ALL: [Self; 4] = [
        Self::LowPass,
        Self::HighPass,
        Self::BandPass,
        Self::BandStop,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::LowPass => "Low Pass",
            Self::HighPass => "High Pass",
            Self::BandPass => "Band Pass",
            Self::BandStop => "Band Stop",
        }
    }

    pub fn is_band(self) -> bool {
        matches!(self, Self::BandPass | Self::BandStop)
    }
}
