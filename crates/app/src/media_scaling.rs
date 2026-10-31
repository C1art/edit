//! The Default Media Scaling preference: how pictures of another size are scaled to the
//! sequence frame when brought in.

use timeline::FrameFit;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MediaScaling {
    None,
    SetToFrameSize,
    SetToFillFrame,
}

impl MediaScaling {
    pub(crate) const ALL: [Self; 3] = [Self::None, Self::SetToFrameSize, Self::SetToFillFrame];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::SetToFrameSize => "Set to Frame Size",
            Self::SetToFillFrame => "Set to Fill Frame",
        }
    }

    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::SetToFrameSize => "frame_size",
            Self::SetToFillFrame => "fill_frame",
        }
    }

    pub(crate) fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|scaling| scaling.key() == key)
    }

    pub(crate) fn fit(self) -> Option<FrameFit> {
        match self {
            Self::None => None,
            Self::SetToFrameSize => Some(FrameFit::Fit),
            Self::SetToFillFrame => Some(FrameFit::Fill),
        }
    }
}
