mod look;
mod transitions;

use std::sync::Arc;

use egui::RichText;
use timeline::{preview, Clip, ClipId, Look, Sequence, SetClipLook, Ticks, TrackKind, TrackRef};

use crate::editor::Editor;
use crate::mixer::Change;

pub use transitions::apply_default_transition;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SideTab {
    #[default]
    EffectControls,
    AudioMixer,
    Scopes,
}

/// The look being dragged, shown live and committed on release.
#[derive(Debug, Default)]
pub struct EffectControls {
    adjusting: Option<(ClipId, Look)>,
}

pub fn ui(editor: &mut Editor, ui: &mut egui::Ui) {
    let sequence = editor.timeline.sequence();
    // Linked video and audio are selected together; the picture has more to adjust.
    let mut selected: Vec<(TrackRef, &Clip)> = editor
        .view
        .selection
        .iter()
        .filter_map(|&id| sequence.clip(id))
        .collect();
    selected.sort_by_key(|(track, _)| track.kind);
    let Some((track, clip)) = selected.first().map(|&(track, clip)| (track, clip.clone())) else {
        ui.label("Select a clip to adjust its motion, opacity, crop, effects and transitions.");
        return;
    };
    ui.label(RichText::new(clip_name(sequence, &clip)).strong());
    if track.kind == TrackKind::Audio {
        transitions::controls(editor, ui, track, &clip);
        return;
    }
    // Keyframes go where the playhead is, kept within the clip.
    let last = clip.end() - Ticks(1);
    let time = clip.source_time(editor.playhead().clamp(clip.start, last));
    let mut look = match &editor.effects.adjusting {
        Some((id, look)) if *id == clip.id => look.clone(),
        _ => clip.look.clone(),
    };
    let output = egui::ScrollArea::vertical()
        .show(ui, |ui| {
            let output = look::controls(ui, &mut look, time);
            transitions::controls(editor, ui, track, &clip);
            output
        })
        .inner;
    if let Some(key) = output.seek {
        editor.seek_program(
            clip.start + (key - clip.source_in).clamp(Ticks::ZERO, last - clip.start),
        );
    }
    let edit = SetClipLook {
        clip: clip.id,
        look,
    };
    match output.change {
        Some(Change::Live) => {
            if let Ok(sequence) = preview(editor.timeline.sequence(), &edit) {
                if let Some(program) = &mut editor.program {
                    program.player.set_sequence(Arc::new(sequence));
                }
            }
            editor.effects.adjusting = Some((edit.clip, edit.look));
        }
        Some(Change::Commit) => {
            editor.effects.adjusting = None;
            editor.apply(edit);
        }
        None => {}
    }
}

fn clip_name<'a>(sequence: &'a Sequence, clip: &Clip) -> &'a str {
    sequence
        .media_item(clip.source)
        .map_or("Missing media", |item| item.name.as_str())
}
