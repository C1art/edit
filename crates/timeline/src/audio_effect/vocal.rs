use serde::{Deserialize, Serialize};

/// What Vocal Enhancer shapes the sound for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum VocalMode {
    /// Deeper voices: less boom, more clarity.
    #[default]
    LowTone,
    /// Higher voices: less harshness, more body.
    HighTone,
    /// Music or other sound under the voice, cut back where speech lives.
    Music,
}

impl VocalMode {
    pub const ALL: [Self; 3] = [Self::LowTone, Self::HighTone, Self::Music];

    pub fn name(self) -> &'static str {
        match self {
            Self::LowTone => "Low Tone",
            Self::HighTone => "High Tone",
            Self::Music => "Music",
        }
    }
}
