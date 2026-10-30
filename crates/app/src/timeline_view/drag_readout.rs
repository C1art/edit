use egui::{Response, Tooltip};
use timeline::{ClipId, Sequence, Ticks};

use super::gesture::PendingEdit;
use super::TimelineView;

impl TimelineView {
    /// Labels a drag by the pointer with how far it moves things and, for a trim or a
    /// stretch, how long the clip becomes, as Premiere does.
    pub(super) fn show_drag_readout(&self, response: &Response, sequence: &Sequence) {
        let Some(drag) = &self.drag else {
            return;
        };
        let Some(pending) = &drag.pending else {
            return;
        };
        let timecode = |time: Ticks| sequence.timecode(time).to_string();
        let (delta, resized) = match pending {
            PendingEdit::Move(edit) => (edit.delta, None),
            PendingEdit::Trim(edit) => (edit.delta, edit.clips.first()),
            PendingEdit::Stretch(edit) => (edit.delta, edit.clips.first()),
            PendingEdit::Roll(edit) => (edit.delta, None),
            PendingEdit::Slip(edit) => (edit.delta, None),
            PendingEdit::Slide(edit) => (edit.delta, None),
            PendingEdit::Transition(edit) => {
                if let Some(transition) = edit.transition {
                    readout(
                        response,
                        format!("Duration {}", timecode(transition.duration)),
                    );
                }
                return;
            }
            PendingEdit::Volume(_) | PendingEdit::Opacity(_) | PendingEdit::Automation(_) => return,
        };
        let sign = if delta >= Ticks::ZERO { "+" } else { "" };
        let mut text = format!("{sign}{}", timecode(delta));
        let duration = resized.and_then(|&clip| resized_duration(drag.preview.as_ref(), clip));
        if let Some(duration) = duration {
            text.push_str(&format!("\nDuration {}", timecode(duration)));
        }
        readout(response, text);
    }
}

fn readout(response: &Response, text: String) {
    Tooltip::always_open(
        response.ctx.clone(),
        response.layer_id,
        response.id,
        egui::PopupAnchor::Pointer,
    )
    .gap(12.0)
    .show(|ui| ui.monospace(text));
}

fn resized_duration(preview: Option<&Sequence>, clip: ClipId) -> Option<Ticks> {
    preview?.clip(clip).map(|(_, clip)| clip.duration)
}
