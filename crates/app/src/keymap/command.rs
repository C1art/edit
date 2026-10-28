use crate::timeline_view::{Direction, Tool};
use crate::transport::TransportKey;

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
    KeyboardShortcuts,
    SpeedDuration,
    AudioGain,
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
    Insert,
    Overwrite,
    AddEdit,
    ExtendEdit,
    SelectAtPlayhead,
    VideoTransition,
    AudioTransition,
    LiftMarked,
    ExtractMarked,
    RippleTrimPrevious,
    RippleTrimNext,
    SelectNearestEditPoint,
    ToggleTrimType,
    TrimBackward,
    TrimForward,
    TrimBackwardMany,
    TrimForwardMany,
    MatchFrame,
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
    MarkClip,
    MarkSelection,
    GoToIn,
    GoToOut,
    AddMarker,
    NextMarker,
    PreviousMarker,
    ClearMarker,
    SelectionTool,
    TrackSelectForwardTool,
    TrackSelectBackwardTool,
    RippleTool,
    RollTool,
    RateStretchTool,
    RazorTool,
    SlipTool,
    SlideTool,
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
    PreviousEdit,
    NextEdit,
    PlayInToOut,
    PlayAround,
    ToggleLoop,
}

impl Command {
    pub const ALL: [Self; 103] = [
        Self::NewProject,
        Self::OpenProject,
        Self::Save,
        Self::SaveAs,
        Self::Import,
        Self::ExportMedia,
        Self::ExportFrame,
        Self::NewTitle,
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
        Self::KeyboardShortcuts,
        Self::SpeedDuration,
        Self::AudioGain,
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
        Self::Insert,
        Self::Overwrite,
        Self::AddEdit,
        Self::ExtendEdit,
        Self::SelectAtPlayhead,
        Self::VideoTransition,
        Self::AudioTransition,
        Self::LiftMarked,
        Self::ExtractMarked,
        Self::RippleTrimPrevious,
        Self::RippleTrimNext,
        Self::SelectNearestEditPoint,
        Self::ToggleTrimType,
        Self::TrimBackward,
        Self::TrimForward,
        Self::TrimBackwardMany,
        Self::TrimForwardMany,
        Self::MatchFrame,
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
        Self::MarkClip,
        Self::MarkSelection,
        Self::GoToIn,
        Self::GoToOut,
        Self::AddMarker,
        Self::NextMarker,
        Self::PreviousMarker,
        Self::ClearMarker,
        Self::SelectionTool,
        Self::TrackSelectForwardTool,
        Self::TrackSelectBackwardTool,
        Self::RippleTool,
        Self::RollTool,
        Self::RateStretchTool,
        Self::RazorTool,
        Self::SlipTool,
        Self::SlideTool,
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
        Self::PreviousEdit,
        Self::NextEdit,
        Self::PlayInToOut,
        Self::PlayAround,
        Self::ToggleLoop,
    ];

    /// How the command is written in the shortcuts file, which must never change or saved
    /// keys for it are lost, and its name.
    fn words(self) -> (&'static str, &'static str) {
        match self {
            Self::NewProject => ("new_project", "New Project"),
            Self::OpenProject => ("open_project", "Open Project"),
            Self::Save => ("save", "Save"),
            Self::SaveAs => ("save_as", "Save As"),
            Self::Import => ("import", "Import"),
            Self::ExportMedia => ("export_media", "Export Media"),
            Self::ExportFrame => ("export_frame", "Export Frame"),
            Self::NewTitle => ("new_title", "New Title"),
            Self::Undo => ("undo", "Undo"),
            Self::Redo => ("redo", "Redo"),
            Self::Cut => ("cut", "Cut"),
            Self::Copy => ("copy", "Copy"),
            Self::Paste => ("paste", "Paste"),
            Self::PasteInsert => ("paste_insert", "Paste Insert"),
            Self::PasteAttributes => ("paste_attributes", "Paste Attributes"),
            Self::Clear => ("clear", "Clear"),
            Self::RippleDelete => ("ripple_delete", "Ripple Delete"),
            Self::CancelDrag => ("cancel_drag", "Cancel Drag"),
            Self::SelectAll => ("select_all", "Select All"),
            Self::DeselectAll => ("deselect_all", "Deselect All"),
            Self::KeyboardShortcuts => ("keyboard_shortcuts", "Keyboard Shortcuts"),
            Self::SpeedDuration => ("speed_duration", "Speed/Duration"),
            Self::AudioGain => ("audio_gain", "Audio Gain"),
            Self::ToggleEnabled => ("toggle_enabled", "Enable"),
            Self::ToggleLink => ("toggle_link", "Link"),
            Self::Group => ("group", "Group"),
            Self::Ungroup => ("ungroup", "Ungroup"),
            Self::JoinThroughEdits => ("join_through_edits", "Join Through Edits"),
            Self::MakeSubclip => ("make_subclip", "Make Subclip"),
            Self::NudgeLeft => ("nudge_left", "Nudge Clip Selection Left One Frame"),
            Self::NudgeRight => ("nudge_right", "Nudge Clip Selection Right One Frame"),
            Self::NudgeLeftFive => ("nudge_left_five", "Nudge Clip Selection Left Five Frames"),
            Self::NudgeRightFive => ("nudge_right_five", "Nudge Clip Selection Right Five Frames"),
            Self::NudgeUp => ("nudge_up", "Nudge Clip Selection Up"),
            Self::NudgeDown => ("nudge_down", "Nudge Clip Selection Down"),
            Self::Insert => ("insert", "Insert"),
            Self::Overwrite => ("overwrite", "Overwrite"),
            Self::AddEdit => ("add_edit", "Add Edit"),
            Self::ExtendEdit => ("extend_edit", "Extend Selected Edit to Playhead"),
            Self::SelectAtPlayhead => ("select_at_playhead", "Select Clip at Playhead"),
            Self::VideoTransition => ("video_transition", "Apply Video Transition"),
            Self::AudioTransition => ("audio_transition", "Apply Audio Transition"),
            Self::LiftMarked => ("lift_marked", "Lift"),
            Self::ExtractMarked => ("extract_marked", "Extract"),
            Self::RippleTrimPrevious => (
                "ripple_trim_previous",
                "Ripple Trim Previous Edit to Playhead",
            ),
            Self::RippleTrimNext => ("ripple_trim_next", "Ripple Trim Next Edit to Playhead"),
            Self::SelectNearestEditPoint => {
                ("select_nearest_edit_point", "Select Nearest Edit Point")
            }
            Self::ToggleTrimType => ("toggle_trim_type", "Toggle Trim Type"),
            Self::TrimBackward => ("trim_backward", "Trim Backward"),
            Self::TrimForward => ("trim_forward", "Trim Forward"),
            Self::TrimBackwardMany => ("trim_backward_many", "Trim Backward Many"),
            Self::TrimForwardMany => ("trim_forward_many", "Trim Forward Many"),
            Self::MatchFrame => ("match_frame", "Match Frame"),
            Self::ToggleSnapping => ("toggle_snapping", "Snap in Timeline"),
            Self::ToggleLinkedSelection => ("toggle_linked_selection", "Linked Selection"),
            Self::ZoomIn => ("zoom_in", "Zoom In"),
            Self::ZoomOut => ("zoom_out", "Zoom Out"),
            Self::ZoomToFit => ("zoom_to_fit", "Zoom to Sequence"),
            Self::IncreaseVideoHeights => {
                ("increase_video_heights", "Increase Video Track Heights")
            }
            Self::DecreaseVideoHeights => {
                ("decrease_video_heights", "Decrease Video Track Heights")
            }
            Self::IncreaseAudioHeights => {
                ("increase_audio_heights", "Increase Audio Track Heights")
            }
            Self::DecreaseAudioHeights => {
                ("decrease_audio_heights", "Decrease Audio Track Heights")
            }
            Self::ExpandAllTracks => ("expand_all_tracks", "Expand All Tracks"),
            Self::MinimizeAllTracks => ("minimize_all_tracks", "Minimize All Tracks"),
            Self::MulticamView => ("multicam_view", "Multi-Camera View"),
            Self::ComparisonView => ("comparison_view", "Comparison View"),
            Self::RenderInToOut => ("render_in_to_out", "Render In to Out"),
            Self::DeleteRenderFiles => ("delete_render_files", "Delete Render Files"),
            Self::MarkIn => ("mark_in", "Mark In"),
            Self::MarkOut => ("mark_out", "Mark Out"),
            Self::ClearInOut => ("clear_in_out", "Clear In and Out"),
            Self::MarkClip => ("mark_clip", "Mark Clip"),
            Self::MarkSelection => ("mark_selection", "Mark Selection"),
            Self::GoToIn => ("go_to_in", "Go to In"),
            Self::GoToOut => ("go_to_out", "Go to Out"),
            Self::AddMarker => ("add_marker", "Add Marker"),
            Self::NextMarker => ("next_marker", "Go to Next Marker"),
            Self::PreviousMarker => ("previous_marker", "Go to Previous Marker"),
            Self::ClearMarker => ("clear_marker", "Clear Marker"),
            Self::SelectionTool => ("selection_tool", "Selection Tool"),
            Self::TrackSelectForwardTool => {
                ("track_select_forward_tool", "Track Select Forward Tool")
            }
            Self::TrackSelectBackwardTool => {
                ("track_select_backward_tool", "Track Select Backward Tool")
            }
            Self::RippleTool => ("ripple_tool", "Ripple Edit Tool"),
            Self::RollTool => ("roll_tool", "Rolling Edit Tool"),
            Self::RateStretchTool => ("rate_stretch_tool", "Rate Stretch Tool"),
            Self::RazorTool => ("razor_tool", "Razor Tool"),
            Self::SlipTool => ("slip_tool", "Slip Tool"),
            Self::SlideTool => ("slide_tool", "Slide Tool"),
            Self::PlayStop => ("play_stop", "Play/Stop"),
            Self::ShuttleLeft => ("shuttle_left", "Shuttle Left"),
            Self::ShuttleStop => ("shuttle_stop", "Shuttle Stop"),
            Self::ShuttleRight => ("shuttle_right", "Shuttle Right"),
            Self::StepBack => ("step_back", "Step Back One Frame"),
            Self::StepForward => ("step_forward", "Step Forward One Frame"),
            Self::StepBackFive => ("step_back_five", "Step Back Five Frames"),
            Self::StepForwardFive => ("step_forward_five", "Step Forward Five Frames"),
            Self::GoToStart => ("go_to_start", "Go to Start"),
            Self::GoToEnd => ("go_to_end", "Go to End"),
            Self::PreviousEdit => ("previous_edit", "Go to Previous Edit Point"),
            Self::NextEdit => ("next_edit", "Go to Next Edit Point"),
            Self::PlayInToOut => ("play_in_to_out", "Play In to Out"),
            Self::PlayAround => ("play_around", "Play Around"),
            Self::ToggleLoop => ("toggle_loop", "Loop Playback"),
        }
    }

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
            | NewTitle => "File",
            Undo | Redo | Cut | Copy | Paste | PasteInsert | PasteAttributes | Clear
            | RippleDelete | CancelDrag | SelectAll | DeselectAll | KeyboardShortcuts => "Edit",
            SpeedDuration | AudioGain | ToggleEnabled | ToggleLink | Group | Ungroup
            | JoinThroughEdits | MakeSubclip | Insert | Overwrite | NudgeLeft | NudgeRight
            | NudgeLeftFive | NudgeRightFive | NudgeUp | NudgeDown => "Clip",
            AddEdit
            | ExtendEdit
            | SelectAtPlayhead
            | VideoTransition
            | AudioTransition
            | LiftMarked
            | ExtractMarked
            | RippleTrimPrevious
            | RippleTrimNext
            | SelectNearestEditPoint
            | ToggleTrimType
            | TrimBackward
            | TrimForward
            | TrimBackwardMany
            | TrimForwardMany
            | MatchFrame
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
            MarkIn | MarkOut | ClearInOut | MarkClip | MarkSelection | GoToIn | GoToOut
            | AddMarker | NextMarker | PreviousMarker | ClearMarker => "Markers",
            SelectionTool
            | TrackSelectForwardTool
            | TrackSelectBackwardTool
            | RippleTool
            | RollTool
            | RateStretchTool
            | RazorTool
            | SlipTool
            | SlideTool => "Tools",
            PlayStop | ShuttleLeft | ShuttleStop | ShuttleRight | StepBack | StepForward
            | StepBackFive | StepForwardFive | GoToStart | GoToEnd | PreviousEdit | NextEdit
            | PlayInToOut | PlayAround | ToggleLoop => "Playback",
        }
    }

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
            _ => return None,
        })
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
