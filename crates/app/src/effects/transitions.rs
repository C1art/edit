use egui::DragValue;
use timeline::{
    nearest_transition_edge, Alignment, Clip, Edge, Rgb, Sequence, SetTransition, Side, Sweep,
    Ticks, TrackKind, TrackRef, Transition, TransitionKind,
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
        super::controls::label_column(ui, label);
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
                    .selected_text(transition.kind.side_name(side))
                    .show_ui(ui, |ui| {
                        for choice in Side::ALL {
                            let name = transition.kind.side_name(choice);
                            ui.selectable_value(&mut side, choice, name);
                        }
                    });
                if side != transition.side() {
                    transition.from = Some(side);
                }
            }
        });
    }
    if let Some(transition) = chosen
        .as_mut()
        .filter(|transition| transition.kind.pattern().is_some())
    {
        wipe_row(ui, label, transition);
    }
    (chosen != current).then_some(chosen)
}

/// Reverse, border width and color, and softness, which only wipes have.
fn wipe_row(ui: &mut egui::Ui, label: &str, transition: &mut Transition) {
    ui.horizontal(|ui| {
        ui.add_space(38.0);
        let mut reversed = transition.sweep == Sweep::Reverse;
        if ui.checkbox(&mut reversed, "Reverse").changed() {
            transition.sweep = if reversed {
                Sweep::Reverse
            } else {
                Sweep::Forward
            };
        }
        ui.label("Border");
        let border = &mut transition.border;
        ui.add(
            DragValue::new(&mut border.width)
                .range(0..=200)
                .suffix(" px"),
        )
        .on_hover_text("Border width");
        let Rgb { r, g, b } = border.color;
        let mut color = [r, g, b];
        let picked = ui
            .push_id((label, "border"), |ui| {
                ui.color_edit_button_srgb(&mut color)
            })
            .inner;
        if picked.changed() {
            let [r, g, b] = color;
            border.color = Rgb::new(r, g, b);
        }
        ui.label("Softness");
        let mut softness = (transition.softness() * 100.0).round() as u8;
        let soften = DragValue::new(&mut softness).range(0..=100).suffix("%");
        if ui.add(soften).changed() {
            transition.softness = Some(softness);
        }
    });
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
    apply_transition(editor, kind, TransitionKind::CrossDissolve);
}

/// Like [`apply_default_transition`], with a transition of `transition` kind.
pub fn apply_transition(editor: &mut Editor, kind: TrackKind, transition: TransitionKind) {
    let targets = &editor.view.targets;
    let track = match kind {
        TrackKind::Video => TrackRef::video(targets.video.unwrap_or(0)),
        TrackKind::Audio => TrackRef::audio(targets.audio.unwrap_or(0)),
    };
    add_transition(editor, track, editor.playhead(), transition);
}

/// Puts a `kind` transition at the cut or clip end on `track` nearest `at`, keeping the
/// settings of one already there.
pub fn add_transition(editor: &mut Editor, track: TrackRef, at: Ticks, kind: TransitionKind) {
    let sequence = editor.timeline.sequence();
    let Some((clip, edge)) = sequence
        .track(track)
        .and_then(|clips| nearest_transition_edge(clips, at))
    else {
        editor.notice(format!("No clip on {track} to add a transition to"));
        return;
    };
    let existing = sequence.clip(clip).and_then(|(_, clip)| match edge {
        Edge::Head => clip.transition_in,
        Edge::Tail => clip.transition_out,
    });
    let transition = Transition {
        kind,
        ..existing.unwrap_or_else(|| default_transition(sequence))
    };
    editor.apply(SetTransition {
        clip,
        edge,
        transition: Some(transition),
    });
}
