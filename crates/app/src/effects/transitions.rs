use egui::DragValue;
use timeline::{
    nearest_transition_edge, Clip, Edge, Sequence, SetTransition, TrackKind, TrackRef, Transition,
    TransitionKind,
};

use crate::editor::Editor;

pub(super) fn controls(editor: &mut Editor, ui: &mut egui::Ui, track: TrackRef, clip: &Clip) {
    let sequence = editor.timeline.sequence();
    let followed = sequence
        .track(track)
        .is_some_and(|track| track.clips.iter().any(|next| next.start == clip.end()));
    let mut edits = Vec::new();
    egui::CollapsingHeader::new("Transitions")
        .default_open(true)
        .show(ui, |ui| {
            for (edge, label, current) in [
                (Edge::Head, "In", clip.transition_in),
                (Edge::Tail, "Out", clip.transition_out),
            ] {
                if edge == Edge::Tail && followed {
                    ui.weak("Out: set on the next clip's In");
                    continue;
                }
                if let Some(transition) = transition_row(ui, sequence, label, current) {
                    edits.push(SetTransition {
                        clip: clip.id,
                        edge,
                        transition,
                    });
                }
            }
        });
    for edit in edits {
        editor.apply(edit);
    }
}

/// Returns the transition to set when the row changed.
fn transition_row(
    ui: &mut egui::Ui,
    sequence: &Sequence,
    label: &str,
    current: Option<Transition>,
) -> Option<Option<Transition>> {
    let (rate, timebase) = (sequence.format.frame_rate, sequence.timebase);
    let mut chosen = current;
    ui.horizontal(|ui| {
        ui.add_sized([30.0, 18.0], egui::Label::new(label));
        let selected = current.map_or("None", |transition| transition.kind.name());
        egui::ComboBox::from_id_salt(label)
            .selected_text(selected)
            .show_ui(ui, |ui| {
                if ui.selectable_label(current.is_none(), "None").clicked() {
                    chosen = None;
                }
                for kind in TransitionKind::ALL {
                    let on = current.is_some_and(|transition| transition.kind == kind);
                    if ui.selectable_label(on, kind.name()).clicked() {
                        chosen = Some(Transition {
                            kind,
                            ..current.unwrap_or_else(|| default_transition(sequence))
                        });
                    }
                }
            });
        if let Some(transition) = &mut chosen {
            let mut frames = rate.frame_at(transition.duration, timebase);
            let response = ui.add(DragValue::new(&mut frames).range(1..=600).suffix(" fr"));
            if response.changed() {
                transition.duration = rate.frame_start(frames, timebase);
            }
        }
    });
    (chosen != current).then_some(chosen)
}

/// A one-second cross dissolve, like Premiere's default; on audio it is a
/// constant-power crossfade.
pub fn default_transition(sequence: &Sequence) -> Transition {
    let rate = sequence.format.frame_rate;
    let frames = i64::from(rate.num().div_ceil(rate.den()));
    Transition {
        kind: TransitionKind::CrossDissolve,
        duration: rate.frame_start(frames, sequence.timebase),
    }
}

/// Applies the default transition at the cut or clip end nearest the playhead on the
/// targeted track of `kind`, or on the first such track when none is targeted.
pub fn apply_default_transition(editor: &mut Editor, kind: TrackKind) {
    let sequence = editor.timeline.sequence();
    let targets = &editor.view.targets;
    let track = match kind {
        TrackKind::Video => TrackRef::video(targets.video.unwrap_or(0)),
        TrackKind::Audio => TrackRef::audio(targets.audio.unwrap_or(0)),
    };
    let Some(edge) = sequence
        .track(track)
        .and_then(|clips| nearest_transition_edge(clips, editor.playhead()))
    else {
        editor.notice(format!("No clip on {track} to add a transition to"));
        return;
    };
    let (clip, edge) = edge;
    let transition = Some(default_transition(sequence));
    editor.apply(SetTransition {
        clip,
        edge,
        transition,
    });
}
