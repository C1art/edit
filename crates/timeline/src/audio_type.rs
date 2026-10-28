use serde::{Deserialize, Serialize};

/// What an audio clip holds, which decides the controls Essential Sound offers for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AudioType {
    Dialogue,
    Music,
    Effects,
    Ambience,
}

impl AudioType {
    pub const ALL: [Self; 4] = [Self::Dialogue, Self::Music, Self::Effects, Self::Ambience];

    pub fn name(self) -> &'static str {
        match self {
            Self::Dialogue => "Dialogue",
            Self::Music => "Music",
            Self::Effects => "SFX",
            Self::Ambience => "Ambience",
        }
    }
}
