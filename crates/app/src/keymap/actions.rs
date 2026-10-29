use timeline::ShapeKind;

use super::Command;
use crate::timeline_view::{Direction, Tool};
use crate::transport::TransportKey;

impl Command {
    pub fn tool(self) -> Option<Tool> {
        Some(match self {
            Self::SelectionTool => Tool::Selection,
            Self::TrackSelectForwardTool => Tool::TrackSelect(Direction::Forward),
            Self::TrackSelectBackwardTool => Tool::TrackSelect(Direction::Backward),
            Self::RippleTool => Tool::Ripple,
            Self::RollTool => Tool::Roll,
            Self::RateStretchTool => Tool::RateStretch,
            Self::RazorTool => Tool::Razor,
            Self::SlipTool => Tool::Slip,
            Self::SlideTool => Tool::Slide,
            Self::HandTool => Tool::Hand,
            Self::ZoomTool => Tool::Zoom,
            Self::TypeTool => Tool::Type,
            Self::RectangleTool => Tool::Shape(ShapeKind::Rectangle),
            Self::EllipseTool => Tool::Shape(ShapeKind::Ellipse),
            _ => return None,
        })
    }

    pub fn for_tool(tool: Tool) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|command| command.tool() == Some(tool))
    }

    /// What the command asks of the focused monitor, for playback commands.
    pub fn transport(self) -> Option<TransportKey> {
        Some(match self {
            Self::PlayStop => TransportKey::Toggle,
            Self::ShuttleLeft => TransportKey::Reverse,
            Self::ShuttleStop => TransportKey::Pause,
            Self::ShuttleRight => TransportKey::Forward,
            Self::StepBack => TransportKey::Step(-1),
            Self::StepForward => TransportKey::Step(1),
            Self::StepBackFive => TransportKey::Step(-5),
            Self::StepForwardFive => TransportKey::Step(5),
            Self::GoToStart => TransportKey::Start,
            Self::GoToEnd => TransportKey::End,
            Self::PreviousEdit => TransportKey::PreviousEdit,
            Self::NextEdit => TransportKey::NextEdit,
            Self::PlayInToOut => TransportKey::PlayInToOut,
            Self::PlayAround => TransportKey::PlayAround,
            Self::ToggleLoop => TransportKey::ToggleLoop,
            _ => return None,
        })
    }
}
