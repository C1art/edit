use super::command::{Command, WORKSPACE_SLOTS};

const WORKSPACE_WORDS: [(&str, &str); WORKSPACE_SLOTS as usize] = [
    ("workspace_1", "Workspace 1"),
    ("workspace_2", "Workspace 2"),
    ("workspace_3", "Workspace 3"),
    ("workspace_4", "Workspace 4"),
    ("workspace_5", "Workspace 5"),
    ("workspace_6", "Workspace 6"),
    ("workspace_7", "Workspace 7"),
    ("workspace_8", "Workspace 8"),
    ("workspace_9", "Workspace 9"),
];

impl Command {
    /// How the command is written in the shortcuts file, which must never change or saved
    /// keys for it are lost, and its name.
    pub(super) fn words(self) -> (&'static str, &'static str) {
        match self {
            Self::NewProject => ("new_project", "New Project"),
            Self::OpenProject => ("open_project", "Open Project"),
            Self::Save => ("save", "Save"),
            Self::SaveAs => ("save_as", "Save As"),
            Self::Import => ("import", "Import"),
            Self::ExportMedia => ("export_media", "Export Media"),
            Self::ExportFrame => ("export_frame", "Export Frame"),
            Self::NewTitle => ("new_title", "New Title"),
            Self::Quit => ("quit", "Quit"),
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
            Self::Find => ("find", "Find"),
            Self::KeyboardShortcuts => ("keyboard_shortcuts", "Keyboard Shortcuts"),
            Self::SpeedDuration => ("speed_duration", "Speed/Duration"),
            Self::AudioGain => ("audio_gain", "Audio Gain"),
            Self::IncreaseClipVolume => ("increase_clip_volume", "Increase Clip Volume"),
            Self::DecreaseClipVolume => ("decrease_clip_volume", "Decrease Clip Volume"),
            Self::IncreaseClipVolumeMany => {
                ("increase_clip_volume_many", "Increase Clip Volume Many")
            }
            Self::DecreaseClipVolumeMany => {
                ("decrease_clip_volume_many", "Decrease Clip Volume Many")
            }
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
            Self::SlipLeft => ("slip_left", "Slip Clip Selection Left One Frame"),
            Self::SlipRight => ("slip_right", "Slip Clip Selection Right One Frame"),
            Self::SlipLeftFive => ("slip_left_five", "Slip Clip Selection Left Five Frames"),
            Self::SlipRightFive => ("slip_right_five", "Slip Clip Selection Right Five Frames"),
            Self::SlideLeft => ("slide_left", "Slide Clip Selection Left One Frame"),
            Self::SlideRight => ("slide_right", "Slide Clip Selection Right One Frame"),
            Self::SlideLeftFive => ("slide_left_five", "Slide Clip Selection Left Five Frames"),
            Self::SlideRightFive => ("slide_right_five", "Slide Clip Selection Right Five Frames"),
            Self::Insert => ("insert", "Insert"),
            Self::Overwrite => ("overwrite", "Overwrite"),
            Self::AddEdit => ("add_edit", "Add Edit"),
            Self::AddEditToAllTracks => ("add_edit_all_tracks", "Add Edit to All Tracks"),
            Self::ExtendEdit => ("extend_edit", "Extend Selected Edit to Playhead"),
            Self::SelectAtPlayhead => ("select_at_playhead", "Select Clip at Playhead"),
            Self::VideoTransition => ("video_transition", "Apply Video Transition"),
            Self::AudioTransition => ("audio_transition", "Apply Audio Transition"),
            Self::TransitionsToSelection => (
                "default_transitions_to_selection",
                "Apply Default Transitions to Selection",
            ),
            Self::LiftMarked => ("lift_marked", "Lift"),
            Self::ExtractMarked => ("extract_marked", "Extract"),
            Self::RippleTrimPrevious => (
                "ripple_trim_previous",
                "Ripple Trim Previous Edit to Playhead",
            ),
            Self::RippleTrimNext => ("ripple_trim_next", "Ripple Trim Next Edit to Playhead"),
            Self::TrimPrevious => ("trim_previous", "Trim Previous Edit to Playhead"),
            Self::TrimNext => ("trim_next", "Trim Next Edit to Playhead"),
            Self::SelectNearestEditPoint => {
                ("select_nearest_edit_point", "Select Nearest Edit Point")
            }
            Self::ToggleTrimType => ("toggle_trim_type", "Toggle Trim Type"),
            Self::TrimBackward => ("trim_backward", "Trim Backward"),
            Self::TrimForward => ("trim_forward", "Trim Forward"),
            Self::TrimBackwardMany => ("trim_backward_many", "Trim Backward Many"),
            Self::TrimForwardMany => ("trim_forward_many", "Trim Forward Many"),
            Self::MatchFrame => ("match_frame", "Match Frame"),
            Self::ReverseMatchFrame => ("reverse_match_frame", "Reverse Match Frame"),
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
            Self::RenderSelection => ("render_selection", "Render Selection"),
            Self::DeleteRenderFiles => ("delete_render_files", "Delete Render Files"),
            Self::NextGapInSequence => ("next_gap_in_sequence", "Go to Next Gap in Sequence"),
            Self::PreviousGapInSequence => {
                ("previous_gap_in_sequence", "Go to Previous Gap in Sequence")
            }
            Self::NextGapInTrack => ("next_gap_in_track", "Go to Next Gap in Track"),
            Self::PreviousGapInTrack => ("previous_gap_in_track", "Go to Previous Gap in Track"),
            Self::MarkIn => ("mark_in", "Mark In"),
            Self::MarkOut => ("mark_out", "Mark Out"),
            Self::ClearInOut => ("clear_in_out", "Clear In and Out"),
            Self::ClearIn => ("clear_in", "Clear In"),
            Self::ClearOut => ("clear_out", "Clear Out"),
            Self::MarkClip => ("mark_clip", "Mark Clip"),
            Self::MarkSelection => ("mark_selection", "Mark Selection"),
            Self::GoToIn => ("go_to_in", "Go to In"),
            Self::GoToOut => ("go_to_out", "Go to Out"),
            Self::AddMarker => ("add_marker", "Add Marker"),
            Self::NextMarker => ("next_marker", "Go to Next Marker"),
            Self::PreviousMarker => ("previous_marker", "Go to Previous Marker"),
            Self::ClearMarker => ("clear_marker", "Clear Marker"),
            Self::ClearAllMarkers => ("clear_all_markers", "Clear All Markers"),
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
            Self::HandTool => ("hand_tool", "Hand Tool"),
            Self::ZoomTool => ("zoom_tool", "Zoom Tool"),
            Self::TypeTool => ("type_tool", "Type Tool"),
            Self::RectangleTool => ("rectangle_tool", "Rectangle Tool"),
            Self::EllipseTool => ("ellipse_tool", "Ellipse Tool"),
            Self::PlayStop => ("play_stop", "Play/Stop"),
            Self::ShuttleLeft => ("shuttle_left", "Shuttle Left"),
            Self::ShuttleStop => ("shuttle_stop", "Shuttle Stop"),
            Self::ShuttleRight => ("shuttle_right", "Shuttle Right"),
            Self::ShuttleSlowLeft => ("shuttle_slow_left", "Shuttle Slow Left"),
            Self::ShuttleSlowRight => ("shuttle_slow_right", "Shuttle Slow Right"),
            Self::StepBack => ("step_back", "Step Back One Frame"),
            Self::StepForward => ("step_forward", "Step Forward One Frame"),
            Self::StepBackFive => ("step_back_five", "Step Back Five Frames"),
            Self::StepForwardFive => ("step_forward_five", "Step Forward Five Frames"),
            Self::GoToStart => ("go_to_start", "Go to Start"),
            Self::GoToEnd => ("go_to_end", "Go to End"),
            Self::GoToSelectedClipStart => {
                ("go_to_selected_clip_start", "Go to Selected Clip Start")
            }
            Self::GoToSelectedClipEnd => ("go_to_selected_clip_end", "Go to Selected Clip End"),
            Self::PreviousEdit => ("previous_edit", "Go to Previous Edit Point"),
            Self::NextEdit => ("next_edit", "Go to Next Edit Point"),
            Self::PlayInToOut => ("play_in_to_out", "Play In to Out"),
            Self::PreviousKeyframe => ("previous_keyframe", "Go to Previous Keyframe"),
            Self::NextKeyframe => ("next_keyframe", "Go to Next Keyframe"),
            Self::AddRemoveKeyframe => ("add_remove_keyframe", "Add/Remove Keyframe"),
            Self::PlayAround => ("play_around", "Play Around"),
            Self::ToggleLoop => ("toggle_loop", "Loop Playback"),
            Self::ToggleAudioScrubbing => ("toggle_audio_scrubbing", "Audio During Scrubbing"),
            Self::Workspace(slot) => WORKSPACE_WORDS
                .get(usize::from(slot))
                .copied()
                .unwrap_or(("workspace", "Workspace")),
            Self::ResetWorkspace => ("reset_workspace", "Reset to Saved Layout"),
            Self::MaximizePanel => ("maximize_panel", "Maximize or Restore Panel Under Cursor"),
            Self::FullScreen => ("toggle_full_screen", "Toggle Full Screen"),
        }
    }
}
