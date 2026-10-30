/// Everything a key can be bound to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Command {
    NewProject,
    OpenProject,
    Save,
    SaveAs,
    Import,
    ExportMedia,
    ExportFrame,
    NewTitle,
    Quit,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    PasteInsert,
    PasteAttributes,
    Clear,
    RippleDelete,
    CancelDrag,
    SelectAll,
    DeselectAll,
    Find,
    KeyboardShortcuts,
    SpeedDuration,
    AudioGain,
    IncreaseClipVolume,
    DecreaseClipVolume,
    IncreaseClipVolumeMany,
    DecreaseClipVolumeMany,
    ToggleEnabled,
    ToggleLink,
    Group,
    Ungroup,
    JoinThroughEdits,
    MakeSubclip,
    NudgeLeft,
    NudgeRight,
    NudgeLeftFive,
    NudgeRightFive,
    NudgeUp,
    NudgeDown,
    SlipLeft,
    SlipRight,
    SlipLeftFive,
    SlipRightFive,
    SlideLeft,
    SlideRight,
    SlideLeftFive,
    SlideRightFive,
    Insert,
    Overwrite,
    AddEdit,
    AddEditToAllTracks,
    ExtendEdit,
    SelectAtPlayhead,
    VideoTransition,
    AudioTransition,
    TransitionsToSelection,
    LiftMarked,
    ExtractMarked,
    RippleTrimPrevious,
    RippleTrimNext,
    TrimPrevious,
    TrimNext,
    SelectNearestEditPoint,
    ToggleTrimType,
    TrimBackward,
    TrimForward,
    TrimBackwardMany,
    TrimForwardMany,
    MatchFrame,
    ReverseMatchFrame,
    ToggleSnapping,
    ToggleLinkedSelection,
    ZoomIn,
    ZoomOut,
    ZoomToFit,
    IncreaseVideoHeights,
    DecreaseVideoHeights,
    IncreaseAudioHeights,
    DecreaseAudioHeights,
    ExpandAllTracks,
    MinimizeAllTracks,
    MulticamView,
    ComparisonView,
    RenderInToOut,
    DeleteRenderFiles,
    MarkIn,
    MarkOut,
    ClearInOut,
    ClearIn,
    ClearOut,
    MarkClip,
    MarkSelection,
    GoToIn,
    GoToOut,
    AddMarker,
    NextMarker,
    PreviousMarker,
    ClearMarker,
    ClearAllMarkers,
    SelectionTool,
    TrackSelectForwardTool,
    TrackSelectBackwardTool,
    RippleTool,
    RollTool,
    RateStretchTool,
    RazorTool,
    SlipTool,
    SlideTool,
    HandTool,
    ZoomTool,
    TypeTool,
    RectangleTool,
    EllipseTool,
    PlayStop,
    ShuttleLeft,
    ShuttleStop,
    ShuttleRight,
    StepBack,
    StepForward,
    StepBackFive,
    StepForwardFive,
    GoToStart,
    GoToEnd,
    GoToSelectedClipStart,
    GoToSelectedClipEnd,
    PreviousEdit,
    NextEdit,
    PreviousKeyframe,
    NextKeyframe,
    PlayInToOut,
    PlayAround,
    ToggleLoop,
    ToggleAudioScrubbing,
    /// Switches to the workspace in this place of the Workspaces menu, from zero.
    Workspace(u8),
    ResetWorkspace,
    MaximizePanel,
    FullScreen,
}

/// Workspaces after these have no command of their own.
pub const WORKSPACE_SLOTS: u8 = 9;

impl Command {
    pub const ALL: [Self; 147] = [
        Self::NewProject,
        Self::OpenProject,
        Self::Save,
        Self::SaveAs,
        Self::Import,
        Self::ExportMedia,
        Self::ExportFrame,
        Self::NewTitle,
        Self::Quit,
        Self::Undo,
        Self::Redo,
        Self::Cut,
        Self::Copy,
        Self::Paste,
        Self::PasteInsert,
        Self::PasteAttributes,
        Self::Clear,
        Self::RippleDelete,
        Self::CancelDrag,
        Self::SelectAll,
        Self::DeselectAll,
        Self::Find,
        Self::KeyboardShortcuts,
        Self::SpeedDuration,
        Self::AudioGain,
        Self::IncreaseClipVolume,
        Self::DecreaseClipVolume,
        Self::IncreaseClipVolumeMany,
        Self::DecreaseClipVolumeMany,
        Self::ToggleEnabled,
        Self::ToggleLink,
        Self::Group,
        Self::Ungroup,
        Self::JoinThroughEdits,
        Self::MakeSubclip,
        Self::NudgeLeft,
        Self::NudgeRight,
        Self::NudgeLeftFive,
        Self::NudgeRightFive,
        Self::NudgeUp,
        Self::NudgeDown,
        Self::SlipLeft,
        Self::SlipRight,
        Self::SlipLeftFive,
        Self::SlipRightFive,
        Self::SlideLeft,
        Self::SlideRight,
        Self::SlideLeftFive,
        Self::SlideRightFive,
        Self::Insert,
        Self::Overwrite,
        Self::AddEdit,
        Self::AddEditToAllTracks,
        Self::ExtendEdit,
        Self::SelectAtPlayhead,
        Self::VideoTransition,
        Self::AudioTransition,
        Self::TransitionsToSelection,
        Self::LiftMarked,
        Self::ExtractMarked,
        Self::RippleTrimPrevious,
        Self::RippleTrimNext,
        Self::TrimPrevious,
        Self::TrimNext,
        Self::SelectNearestEditPoint,
        Self::ToggleTrimType,
        Self::TrimBackward,
        Self::TrimForward,
        Self::TrimBackwardMany,
        Self::TrimForwardMany,
        Self::MatchFrame,
        Self::ReverseMatchFrame,
        Self::ToggleSnapping,
        Self::ToggleLinkedSelection,
        Self::ZoomIn,
        Self::ZoomOut,
        Self::ZoomToFit,
        Self::IncreaseVideoHeights,
        Self::DecreaseVideoHeights,
        Self::IncreaseAudioHeights,
        Self::DecreaseAudioHeights,
        Self::ExpandAllTracks,
        Self::MinimizeAllTracks,
        Self::MulticamView,
        Self::ComparisonView,
        Self::RenderInToOut,
        Self::DeleteRenderFiles,
        Self::MarkIn,
        Self::MarkOut,
        Self::ClearInOut,
        Self::ClearIn,
        Self::ClearOut,
        Self::MarkClip,
        Self::MarkSelection,
        Self::GoToIn,
        Self::GoToOut,
        Self::AddMarker,
        Self::NextMarker,
        Self::PreviousMarker,
        Self::ClearMarker,
        Self::ClearAllMarkers,
        Self::SelectionTool,
        Self::TrackSelectForwardTool,
        Self::TrackSelectBackwardTool,
        Self::RippleTool,
        Self::RollTool,
        Self::RateStretchTool,
        Self::RazorTool,
        Self::SlipTool,
        Self::SlideTool,
        Self::HandTool,
        Self::ZoomTool,
        Self::TypeTool,
        Self::RectangleTool,
        Self::EllipseTool,
        Self::PlayStop,
        Self::ShuttleLeft,
        Self::ShuttleStop,
        Self::ShuttleRight,
        Self::StepBack,
        Self::StepForward,
        Self::StepBackFive,
        Self::StepForwardFive,
        Self::GoToStart,
        Self::GoToEnd,
        Self::GoToSelectedClipStart,
        Self::GoToSelectedClipEnd,
        Self::PreviousEdit,
        Self::NextEdit,
        Self::PreviousKeyframe,
        Self::NextKeyframe,
        Self::PlayInToOut,
        Self::PlayAround,
        Self::ToggleLoop,
        Self::ToggleAudioScrubbing,
        Self::Workspace(0),
        Self::Workspace(1),
        Self::Workspace(2),
        Self::Workspace(3),
        Self::Workspace(4),
        Self::Workspace(5),
        Self::Workspace(6),
        Self::Workspace(7),
        Self::Workspace(8),
        Self::ResetWorkspace,
        Self::MaximizePanel,
        Self::FullScreen,
    ];

    pub fn id(self) -> &'static str {
        self.words().0
    }

    pub fn name(self) -> &'static str {
        self.words().1
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|command| command.id() == id)
    }

    pub fn category(self) -> &'static str {
        use Command::*;
        match self {
            NewProject | OpenProject | Save | SaveAs | Import | ExportMedia | ExportFrame
            | NewTitle | Quit => "File",
            Undo | Redo | Cut | Copy | Paste | PasteInsert | PasteAttributes | Clear
            | RippleDelete | CancelDrag | SelectAll | DeselectAll | Find | KeyboardShortcuts => {
                "Edit"
            }
            SpeedDuration
            | AudioGain
            | IncreaseClipVolume
            | DecreaseClipVolume
            | IncreaseClipVolumeMany
            | DecreaseClipVolumeMany
            | ToggleEnabled
            | ToggleLink
            | Group
            | Ungroup
            | JoinThroughEdits
            | MakeSubclip
            | Insert
            | Overwrite
            | NudgeLeft
            | NudgeRight
            | NudgeLeftFive
            | NudgeRightFive
            | NudgeUp
            | NudgeDown
            | SlipLeft
            | SlipRight
            | SlipLeftFive
            | SlipRightFive
            | SlideLeft
            | SlideRight
            | SlideLeftFive
            | SlideRightFive => "Clip",
            AddEdit
            | AddEditToAllTracks
            | ExtendEdit
            | SelectAtPlayhead
            | VideoTransition
            | AudioTransition
            | TransitionsToSelection
            | LiftMarked
            | ExtractMarked
            | RippleTrimPrevious
            | RippleTrimNext
            | TrimPrevious
            | TrimNext
            | SelectNearestEditPoint
            | ToggleTrimType
            | TrimBackward
            | TrimForward
            | TrimBackwardMany
            | TrimForwardMany
            | MatchFrame
            | ReverseMatchFrame
            | ToggleSnapping
            | ToggleLinkedSelection
            | ZoomIn
            | ZoomOut
            | ZoomToFit
            | IncreaseVideoHeights
            | DecreaseVideoHeights
            | IncreaseAudioHeights
            | DecreaseAudioHeights
            | ExpandAllTracks
            | MinimizeAllTracks
            | MulticamView
            | ComparisonView
            | RenderInToOut
            | DeleteRenderFiles => "Sequence",
            MarkIn | MarkOut | ClearInOut | ClearIn | ClearOut | MarkClip | MarkSelection
            | GoToIn | GoToOut | AddMarker | NextMarker | PreviousMarker | ClearMarker
            | ClearAllMarkers => "Markers",
            SelectionTool
            | TrackSelectForwardTool
            | TrackSelectBackwardTool
            | RippleTool
            | RollTool
            | RateStretchTool
            | RazorTool
            | SlipTool
            | SlideTool
            | HandTool
            | ZoomTool
            | TypeTool
            | RectangleTool
            | EllipseTool => "Tools",
            PlayStop
            | ShuttleLeft
            | ShuttleStop
            | ShuttleRight
            | StepBack
            | StepForward
            | StepBackFive
            | StepForwardFive
            | GoToStart
            | GoToEnd
            | GoToSelectedClipStart
            | GoToSelectedClipEnd
            | PreviousEdit
            | NextEdit
            | PreviousKeyframe
            | NextKeyframe
            | PlayInToOut
            | PlayAround
            | ToggleLoop
            | ToggleAudioScrubbing => "Playback",
            Workspace(_) | ResetWorkspace | MaximizePanel | FullScreen => "Window",
        }
    }
}
