use crate::{Look, Param};

/// The property of a video clip whose keyframes the timeline draws as a line across it,
/// as Premiere's Show Clip Keyframes picks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ClipLine {
    #[default]
    Opacity,
    Scale,
    Rotation,
    PositionX,
    PositionY,
}

impl ClipLine {
    pub const ALL: [Self; 5] = [
        Self::Opacity,
        Self::Scale,
        Self::Rotation,
        Self::PositionX,
        Self::PositionY,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Opacity => "Opacity",
            Self::Scale => "Scale",
            Self::Rotation => "Rotation",
            Self::PositionX => "Position X",
            Self::PositionY => "Position Y",
        }
    }

    pub fn param(self, look: &Look) -> &Param {
        match self {
            Self::Opacity => &look.opacity,
            Self::Scale => &look.motion.scale,
            Self::Rotation => &look.motion.rotation,
            Self::PositionX => &look.motion.position[0],
            Self::PositionY => &look.motion.position[1],
        }
    }

    pub fn param_mut(self, look: &mut Look) -> &mut Param {
        match self {
            Self::Opacity => &mut look.opacity,
            Self::Scale => &mut look.motion.scale,
            Self::Rotation => &mut look.motion.rotation,
            Self::PositionX => &mut look.motion.position[0],
            Self::PositionY => &mut look.motion.position[1],
        }
    }
}
