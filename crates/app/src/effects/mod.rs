mod generated;
mod look;
mod transitions;

use std::sync::Arc;

use egui::RichText;
use timeline::{
    preview, Clip, ClipId, Edit, Generator, Look, Sequence, SetClipLook, SetGenerator, SourceId,
    Ticks, TrackKind, TrackRef,
};

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

/// The look being dragged, or the title being typed, shown live and committed on
/// release.
#[derive(Debug, Default)]
pub struct EffectControls {
    adjusting: Option<(ClipId, Look)>,
    drawing: Option<(SourceId, Generator)>,
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
    let mut drawing = sequence
        .media_item(clip.source)
        .and_then(|item| item.generator.clone())
        .map(|generator| match &editor.effects.drawing {
            Some((id, drawing)) if *id == clip.source => drawing.clone(),
            _ => generator,
        });
    let mut drawing_change = None;
    let output = egui::ScrollArea::vertical()
        .show(ui, |ui| {
            if let Some(generator) = &mut drawing {
                drawing_change = generated::controls(ui, generator);
            }
            let output = look::controls(ui, &mut look, time);
            transitions::controls(editor, ui, track, &clip);
            output
        })
        .inner;
    if let Some(key) = output.seek {
        editor.seek_program(clip.position_of(key).clamp(clip.start, last));
    }
    if let (Some(generator), Some(change)) = (drawing, drawing_change) {
        let edit = SetGenerator {
            source: clip.source,
            generator,
        };
        editor.effects.drawing = None;
        match change {
            Change::Live => {
                show_live(editor, &edit);
                editor.effects.drawing = Some((edit.source, edit.generator));
            }
            Change::Commit => {
                editor.apply(edit);
            }
        }
    }
    let edit = SetClipLook {
        clip: clip.id,
        look,
    };
    match output.change {
        Some(Change::Live) => {
            show_live(editor, &edit);
            editor.effects.adjusting = Some((edit.clip, edit.look));
        }
        Some(Change::Commit) => {
            editor.effects.adjusting = None;
            editor.apply(edit);
        }
        None => {}
    }
}

/// Shows the program as `edit` would leave it, without adding to the undo history.
fn show_live(editor: &mut Editor, edit: &impl Edit) {
    if let Ok(sequence) = preview(editor.timeline.sequence(), edit) {
        if let Some(program) = &mut editor.program {
            program.player.set_sequence(Arc::new(sequence));
        }
    }
}

fn clip_name<'a>(sequence: &'a Sequence, clip: &Clip) -> &'a str {
    sequence
        .media_item(clip.source)
        .map_or("Missing media", |item| item.name.as_str())
}
