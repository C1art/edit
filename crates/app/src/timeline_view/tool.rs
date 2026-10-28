use egui::Key;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    #[default]
    Selection,
    Ripple,
    Roll,
    Razor,
    Slip,
    Slide,
}

impl Tool {
    pub const ALL: [Self; 6] = [
        Self::Selection,
        Self::Ripple,
        Self::Roll,
        Self::Razor,
        Self::Slip,
        Self::Slide,
    ];

    pub fn key(self) -> Key {
        match self {
            Self::Selection => Key::V,
            Self::Ripple => Key::B,
            Self::Roll => Key::N,
            Self::Razor => Key::C,
            Self::Slip => Key::Y,
            Self::Slide => Key::U,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Selection => "Select",
            Self::Ripple => "Ripple",
            Self::Roll => "Roll",
            Self::Razor => "Razor",
            Self::Slip => "Slip",
            Self::Slide => "Slide",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Self::Selection => "Move clips; drag an edge to trim (V)",
            Self::Ripple => "Drag an edge to trim and shift what follows (B)",
            Self::Roll => "Drag a cut to move it between two clips (N)",
            Self::Razor => "Click a clip to cut it (C)",
            Self::Slip => "Drag a clip to change its source range (Y)",
            Self::Slide => "Drag a clip between its neighbours (U)",
        }
    }
}
