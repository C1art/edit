use egui::Response;
use timeline::{Sequence, Ticks, Timecode};

use super::layout::{self, Hit};
use super::scale::TimeScale;
use super::{paint, TimelineView};

impl TimelineView {
    /// Names the clip under a resting pointer with where it starts and ends, as
    /// Premiere does, since a short clip has no room to show it.
    pub(super) fn show_clip_tip(
        &self,
        response: &Response,
        sequence: &Sequence,
        scale: &TimeScale,
        top: f32,
    ) {
        if self.drag.is_some() {
            return;
        }
        let Some(pos) = response.hover_pos() else {
            return;
        };
        let rows = layout::rows(sequence, &self.heights, top);
        let (Hit::Body { clip, .. } | Hit::Edge { clip, .. }) =
            layout::hit(sequence, &rows, scale, pos)
        else {
            return;
        };
        let Some((_, clip)) = sequence.clip(clip) else {
            return;
        };
        let timecode = |time: Ticks| {
            Timecode::new(time, sequence.timebase, sequence.format.frame_rate).to_string()
        };
        let text = format!(
            "Start     {}\nEnd       {}\nDuration  {}",
            timecode(clip.start),
            timecode(clip.end()),
            timecode(clip.duration)
        );
        let name = paint::clip_name(sequence, clip);
        response.clone().on_hover_ui_at_pointer(|ui| {
            ui.strong(name);
            ui.monospace(text);
        });
    }
}
