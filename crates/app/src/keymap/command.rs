use egui::Key;

use super::Chord;
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
    MatchFrame,
    ToggleSnapping,
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
    MarkIn,
    MarkOut,
    ClearInOut,
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
}

impl Command {
    pub const ALL: [Self; 85] = [
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
        Self::MatchFrame,
        Self::ToggleSnapping,
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
        Self::MarkIn,
        Self::MarkOut,
        Self::ClearInOut,
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
            Self::MatchFrame => ("match_frame", "Match Frame"),
            Self::ToggleSnapping => ("toggle_snapping", "Snap in Timeline"),
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
            Self::MarkIn => ("mark_in", "Mark In"),
            Self::MarkOut => ("mark_out", "Mark Out"),
            Self::ClearInOut => ("clear_in_out", "Clear In and Out"),
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
            SpeedDuration | AudioGain | ToggleEnabled | ToggleLink | MakeSubclip | Insert
            | Overwrite | NudgeLeft | NudgeRight | NudgeLeftFive | NudgeRightFive | NudgeUp
            | NudgeDown => "Clip",
            AddEdit | ExtendEdit | SelectAtPlayhead | VideoTransition | AudioTransition
            | LiftMarked | ExtractMarked | RippleTrimPrevious | RippleTrimNext | MatchFrame
            | ToggleSnapping | ZoomIn | ZoomOut | ZoomToFit | IncreaseVideoHeights
            | DecreaseVideoHeights | IncreaseAudioHeights | DecreaseAudioHeights
            | ExpandAllTracks | MinimizeAllTracks | MulticamView => "Sequence",
            MarkIn | MarkOut | ClearInOut | GoToIn | GoToOut | AddMarker | NextMarker
            | PreviousMarker | ClearMarker => "Markers",
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
            | StepBackFive | StepForwardFive | GoToStart | GoToEnd | PreviousEdit | NextEdit => {
                "Playback"
            }
        }
    }

    /// Premiere Pro's default keys for Windows.
    pub fn defaults(self) -> Vec<Chord> {
        let ctrl = |key| Chord::new(true, false, false, key);
        let ctrl_shift = |key| Chord::new(true, true, false, key);
        let ctrl_alt = |key| Chord::new(true, false, true, key);
        let shift = |key| Chord::new(false, true, false, key);
        let plain = |key| Chord::new(false, false, false, key);
        let alt = |key| Chord::new(false, false, true, key);
        let alt_shift = |key| Chord::new(false, true, true, key);
        match self {
            Self::NewProject => vec![ctrl_alt(Key::N)],
            Self::OpenProject => vec![ctrl(Key::O)],
            Self::Save => vec![ctrl(Key::S)],
            Self::SaveAs => vec![ctrl_shift(Key::S)],
            Self::Import => vec![ctrl(Key::I)],
            Self::ExportMedia => vec![ctrl(Key::M)],
            Self::ExportFrame => vec![ctrl_shift(Key::E)],
            Self::NewTitle => vec![ctrl(Key::T)],
            Self::Undo => vec![ctrl(Key::Z)],
            Self::Redo => vec![ctrl_shift(Key::Z), ctrl(Key::Y)],
            Self::Cut => vec![ctrl(Key::X)],
            Self::Copy => vec![ctrl(Key::C)],
            Self::Paste => vec![ctrl(Key::V)],
            Self::PasteInsert => vec![ctrl_shift(Key::V)],
            Self::PasteAttributes => vec![ctrl_alt(Key::V)],
            Self::Clear => vec![plain(Key::Delete), plain(Key::Backspace)],
            Self::RippleDelete => vec![shift(Key::Delete)],
            Self::CancelDrag => vec![plain(Key::Escape)],
            Self::SelectAll => vec![ctrl(Key::A)],
            Self::DeselectAll => vec![ctrl_shift(Key::A)],
            Self::KeyboardShortcuts => vec![ctrl_alt(Key::K)],
            Self::SpeedDuration => vec![ctrl(Key::R)],
            Self::AudioGain => vec![plain(Key::G)],
            Self::ToggleEnabled => vec![shift(Key::E)],
            Self::ToggleLink => vec![ctrl(Key::L)],
            Self::MakeSubclip => vec![ctrl(Key::U)],
            Self::NudgeLeft => vec![alt(Key::ArrowLeft)],
            Self::NudgeRight => vec![alt(Key::ArrowRight)],
            Self::NudgeLeftFive => vec![alt_shift(Key::ArrowLeft)],
            Self::NudgeRightFive => vec![alt_shift(Key::ArrowRight)],
            Self::NudgeUp => vec![alt(Key::ArrowUp)],
            Self::NudgeDown => vec![alt(Key::ArrowDown)],
            Self::Insert => vec![plain(Key::Comma)],
            Self::Overwrite => vec![plain(Key::Period)],
            Self::AddEdit => vec![ctrl(Key::K)],
            Self::ExtendEdit => vec![plain(Key::E)],
            Self::SelectAtPlayhead => vec![plain(Key::D)],
            Self::VideoTransition => vec![ctrl(Key::D)],
            Self::AudioTransition => vec![ctrl_shift(Key::D)],
            Self::LiftMarked => vec![plain(Key::Semicolon)],
            Self::ExtractMarked => vec![plain(Key::Quote)],
            Self::RippleTrimPrevious => vec![plain(Key::Q)],
            Self::RippleTrimNext => vec![plain(Key::W)],
            Self::MatchFrame => vec![plain(Key::F)],
            Self::ToggleSnapping => vec![plain(Key::S)],
            Self::ZoomIn => vec![plain(Key::Equals), plain(Key::Plus)],
            Self::ZoomOut => vec![plain(Key::Minus)],
            Self::ZoomToFit => vec![plain(Key::Backslash)],
            Self::IncreaseVideoHeights => vec![ctrl(Key::Equals)],
            Self::DecreaseVideoHeights => vec![ctrl(Key::Minus)],
            Self::IncreaseAudioHeights => vec![alt(Key::Equals)],
            Self::DecreaseAudioHeights => vec![alt(Key::Minus)],
            Self::ExpandAllTracks => vec![shift(Key::Plus)],
            Self::MinimizeAllTracks => vec![shift(Key::Minus)],
            Self::MulticamView => vec![shift(Key::Num0)],
            Self::MarkIn => vec![plain(Key::I)],
            Self::MarkOut => vec![plain(Key::O)],
            Self::ClearInOut => vec![ctrl_shift(Key::X)],
            Self::GoToIn => vec![shift(Key::I)],
            Self::GoToOut => vec![shift(Key::O)],
            Self::AddMarker => vec![plain(Key::M)],
            Self::NextMarker => vec![shift(Key::M)],
            Self::PreviousMarker => vec![ctrl_shift(Key::M)],
            Self::ClearMarker => vec![ctrl_alt(Key::M)],
            Self::SelectionTool => vec![plain(Key::V)],
            Self::TrackSelectForwardTool => vec![plain(Key::A)],
            Self::TrackSelectBackwardTool => vec![shift(Key::A)],
            Self::RippleTool => vec![plain(Key::B)],
            Self::RollTool => vec![plain(Key::N)],
            Self::RateStretchTool => vec![plain(Key::R)],
            Self::RazorTool => vec![plain(Key::C)],
            Self::SlipTool => vec![plain(Key::Y)],
            Self::SlideTool => vec![plain(Key::U)],
            Self::PlayStop => vec![plain(Key::Space)],
            Self::ShuttleLeft => vec![plain(Key::J)],
            Self::ShuttleStop => vec![plain(Key::K)],
            Self::ShuttleRight => vec![plain(Key::L)],
            Self::StepBack => vec![plain(Key::ArrowLeft)],
            Self::StepForward => vec![plain(Key::ArrowRight)],
            Self::StepBackFive => vec![shift(Key::ArrowLeft)],
            Self::StepForwardFive => vec![shift(Key::ArrowRight)],
            Self::GoToStart => vec![plain(Key::Home)],
            Self::GoToEnd => vec![plain(Key::End)],
            Self::PreviousEdit => vec![plain(Key::ArrowUp)],
            Self::NextEdit => vec![plain(Key::ArrowDown)],
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
            _ => return None,
        })
    }
}
