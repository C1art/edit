use timeline::ShapeKind;

/// Which way along the timeline a track select tool picks clips.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Backward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    #[default]
    Selection,
    TrackSelect(Direction),
    Ripple,
    Roll,
    RateStretch,
    Slip,
    Slide,
    /// Drag to pan the view.
    Hand,
    /// Click to zoom in, Alt-click to zoom out.
    Zoom,
    /// Click the program picture to type a title there.
    Type,
    /// Drag on the program picture to draw a shape.
    Shape(ShapeKind),
}

impl Tool {
    /// Every tool, grouped as the toolbar shows them: one button per group.
    pub const GROUPS: [&'static [Self]; 6] = [
        &[Self::Selection],
        &[
            Self::TrackSelect(Direction::Forward),
            Self::TrackSelect(Direction::Backward),
        ],
        &[Self::Ripple, Self::Roll, Self::RateStretch],
        &[Self::Slip, Self::Slide],
        &[Self::Hand, Self::Zoom],
        &[
            Self::Type,
            Self::Shape(ShapeKind::Rectangle),
            Self::Shape(ShapeKind::Ellipse),
        ],
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Selection => "Select",
            Self::TrackSelect(Direction::Forward) => "Track Fwd",
            Self::TrackSelect(Direction::Backward) => "Track Back",
            Self::Ripple => "Ripple",
            Self::Roll => "Roll",
            Self::RateStretch => "Stretch",
            Self::Slip => "Slip",
            Self::Slide => "Slide",
            Self::Hand => "Hand",
            Self::Zoom => "Zoom",
            Self::Type => "Type",
            Self::Shape(kind) => kind.name(),
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Self::Selection => "Move clips; drag an edge to trim",
            Self::TrackSelect(Direction::Forward) => {
                "Select every clip from here on, or with Shift on one track"
            }
            Self::TrackSelect(Direction::Backward) => {
                "Select every clip up to here, or with Shift on one track"
            }
            Self::Ripple => "Drag an edge to trim and shift what follows",
            Self::Roll => "Drag a cut to move it between two clips",
            Self::RateStretch => "Drag an edge to change a clip's speed to fit",
            Self::Slip => "Drag a clip to change its source range",
            Self::Slide => "Drag a clip between its neighbours",
            Self::Hand => "Drag to scroll the timeline",
            Self::Zoom => "Click to zoom in, Alt+click to zoom out",
            Self::Type => "Click the program picture to type a title there",
            Self::Shape(ShapeKind::Rectangle) => {
                "Drag on the program picture to draw a rectangle; Shift for a square"
            }
            Self::Shape(ShapeKind::Ellipse) => {
                "Drag on the program picture to draw an ellipse; Shift for a circle"
            }
        }
    }

    /// What the tool does on the timeline, where the graphics tools select.
    pub fn on_timeline(self) -> Self {
        match self {
            Self::Type | Self::Shape(_) => Self::Selection,
            tool => tool,
        }
    }
}
