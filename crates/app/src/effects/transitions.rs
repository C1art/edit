use egui::DragValue;
use timeline::{
    nearest_transition_edge, Alignment, Clip, Edge, Sequence, SetTransition, Side, TrackKind,
    TrackRef, Transition, TransitionKind,
};

use crate::editor::Editor;

pub(super) fn controls(editor: &mut Editor, ui: &mut egui::Ui, track: TrackRef, clip: &Clip) {
    let sequence = editor.timeline.sequence();
    let neighbours = sequence.track(track).map(|track| {
        let at = |time| track.clips.iter().any(|other| other.end() == time);
        (
            at(clip.start),
            track.clips.iter().any(|next| next.start == clip.end()),
        )
    });
    let (at_cut, followed) = neighbours.unwrap_or_default();
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
                let at_cut = edge == Edge::Head && at_cut;
                let row = transition_row(ui, sequence, track.kind, label, current, at_cut);
                if let Some(transition) = row {
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
    track: TrackKind,
    label: &str,
    current: Option<Transition>,
    at_cut: bool,
) -> Option<Option<Transition>> {
    let (rate, timebase) = (sequence.format.frame_rate, sequence.timebase);
    let (kinds, name): (&[TransitionKind], fn(TransitionKind) -> &'static str) = match track {
        TrackKind::Video => (&TransitionKind::VIDEO, TransitionKind::name),
        TrackKind::Audio => (&TransitionKind::AUDIO, TransitionKind::audio_name),
    };
    let mut chosen = current;
    ui.horizontal(|ui| {
        ui.add_sized([30.0, 18.0], egui::Label::new(label));
        let selected = current.map_or("None", |transition| name(transition.kind));
        egui::ComboBox::from_id_salt(label)
            .selected_text(selected)
            .show_ui(ui, |ui| {
                if ui.selectable_label(current.is_none(), "None").clicked() {
                    chosen = None;
                }
                for &kind in kinds {
                    let on = current.is_some_and(|transition| transition.kind == kind);
                    if ui.selectable_label(on, name(kind)).clicked() {
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
    if let Some(transition) = chosen
        .as_mut()
        .filter(|transition| at_cut || transition.kind.has_direction())
    {
        ui.horizontal(|ui| {
            ui.add_space(38.0);
            if at_cut {
                egui::ComboBox::from_id_salt((label, "alignment"))
                    .selected_text(transition.alignment.name())
                    .show_ui(ui, |ui| {
                        for alignment in Alignment::ALL {
                            ui.selectable_value(
                                &mut transition.alignment,
                                alignment,
                                alignment.name(),
                            );
                        }
                    });
            }
            if transition.kind.has_direction() {
                let mut side = transition.side();
                egui::ComboBox::from_id_salt((label, "side"))
                    .selected_text(side.name())
                    .show_ui(ui, |ui| {
                        for choice in Side::ALL {
                            ui.selectable_value(&mut side, choice, choice.name());
                        }
                    });
                if side != transition.side() {
                    transition.from = Some(side);
                }
            }
        });
    }
    (chosen != current).then_some(chosen)
}

/// A one-second cross dissolve, like Premiere's default; on audio it is a
/// constant-power crossfade.
pub fn default_transition(sequence: &Sequence) -> Transition {
    let rate = sequence.format.frame_rate;
    let frames = i64::from(rate.num().div_ceil(rate.den()));
    Transition::new(
        TransitionKind::CrossDissolve,
        rate.frame_start(frames, sequence.timebase),
    )
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
