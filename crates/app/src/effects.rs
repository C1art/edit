use std::ops::RangeInclusive;
use std::sync::Arc;

use egui::{DragValue, RichText};
use timeline::{
    nearest_transition_edge, preview, Clip, ClipId, Edge, Look, Param, Sequence, SetClipLook,
    SetTransition, TrackKind, TrackRef, Transition, TransitionKind, VideoEffect,
};

use crate::editor::Editor;
use crate::mixer::{response_state, Change};

const DEFAULT_BLURRINESS: i32 = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SideTab {
    #[default]
    EffectControls,
    AudioMixer,
}

/// The look being dragged, shown live and committed on release.
#[derive(Debug, Default)]
pub struct EffectControls {
    adjusting: Option<(ClipId, Look)>,
}

pub fn ui(editor: &mut Editor, ui: &mut egui::Ui) {
    let sequence = editor.timeline.sequence();
    let selected = editor.view.selection.iter().find_map(|&id| {
        sequence
            .clip(id)
            .filter(|(track, _)| track.kind == TrackKind::Video)
            .map(|(track, clip)| (track, clip.clone()))
    });
    let Some((track, clip)) = selected else {
        ui.label(
            "Select a video clip to adjust its motion, opacity, crop, effects and transitions.",
        );
        return;
    };
    let name = sequence
        .media_item(clip.source)
        .map_or("Missing media", |item| item.name.as_str());
    ui.label(RichText::new(name).strong());
    let mut look = match &editor.effects.adjusting {
        Some((id, look)) if *id == clip.id => look.clone(),
        _ => clip.look.clone(),
    };
    let change = egui::ScrollArea::vertical()
        .show(ui, |ui| {
            let change = look_controls(ui, &mut look);
            transition_controls(editor, ui, track, &clip);
            change
        })
        .inner;
    let edit = SetClipLook {
        clip: clip.id,
        look,
    };
    match change {
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

fn look_controls(ui: &mut egui::Ui, look: &mut Look) -> Option<Change> {
    let mut change = None;
    egui::CollapsingHeader::new("Motion")
        .default_open(true)
        .show(ui, |ui| {
            let [x, y] = &mut look.motion.position;
            change = param(ui, "Position X", x, -10_000.0..=10_000.0, "", change);
            change = param(ui, "Position Y", y, -10_000.0..=10_000.0, "", change);
            let motion = &mut look.motion;
            change = param(ui, "Scale", &mut motion.scale, 0.0..=1_000.0, "%", change);
            change = param(
                ui,
                "Rotation",
                &mut motion.rotation,
                -3_600.0..=3_600.0,
                "°",
                change,
            );
        });
    egui::CollapsingHeader::new("Opacity")
        .default_open(true)
        .show(ui, |ui| {
            change = param(ui, "Opacity", &mut look.opacity, 0.0..=100.0, "%", change);
        });
    egui::CollapsingHeader::new("Crop").show(ui, |ui| {
        let crop = &mut look.crop;
        for (label, side) in [
            ("Left", &mut crop.left),
            ("Top", &mut crop.top),
            ("Right", &mut crop.right),
            ("Bottom", &mut crop.bottom),
        ] {
            change = param(ui, label, side, 0.0..=100.0, "%", change);
        }
    });
    egui::CollapsingHeader::new("Effects")
        .default_open(true)
        .show(ui, |ui| {
            let mut removed = None;
            for (index, effect) in look.effects.iter_mut().enumerate() {
                match effect {
                    VideoEffect::GaussianBlur { blurriness } => {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Gaussian Blur").strong());
                            if ui.small_button("Remove").clicked() {
                                removed = Some(index);
                            }
                        });
                        change = param(ui, "Blurriness", blurriness, 0.0..=500.0, "", change);
                    }
                }
            }
            if let Some(index) = removed {
                look.effects.remove(index);
                change = Some(Change::Commit);
            }
            ui.menu_button("Add Effect", |ui| {
                if ui.button("Gaussian Blur").clicked() {
                    look.effects.push(VideoEffect::GaussianBlur {
                        blurriness: Param::new(DEFAULT_BLURRINESS),
                    });
                    change = Some(Change::Commit);
                }
            });
        });
    ui.add_space(4.0);
    if ui.button("Reset Effects").clicked() && *look != Look::default() {
        *look = Look::default();
        change = Some(Change::Commit);
    }
    change
}

fn param(
    ui: &mut egui::Ui,
    label: &str,
    param: &mut Param,
    range: RangeInclusive<f32>,
    suffix: &str,
    previous: Option<Change>,
) -> Option<Change> {
    ui.horizontal(|ui| {
        ui.add_sized([80.0, 18.0], egui::Label::new(label));
        let mut value = param.value();
        let response = ui.add(
            DragValue::new(&mut value)
                .range(range)
                .speed(0.5)
                .max_decimals(2)
                .suffix(suffix),
        );
        if response.changed() {
            *param = Param::from_value(value);
        }
        response_state(&response, previous)
    })
    .inner
}

fn transition_controls(editor: &mut Editor, ui: &mut egui::Ui, track: TrackRef, clip: &Clip) {
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

/// A one-second cross dissolve, like Premiere's default.
pub fn default_transition(sequence: &Sequence) -> Transition {
    let rate = sequence.format.frame_rate;
    let frames = i64::from(rate.num().div_ceil(rate.den()));
    Transition {
        kind: TransitionKind::CrossDissolve,
        duration: rate.frame_start(frames, sequence.timebase),
    }
}

/// Applies the default transition at the cut or clip end nearest the playhead on the
/// targeted video track, or on V1 when none is targeted.
pub fn apply_default_transition(editor: &mut Editor) {
    let sequence = editor.timeline.sequence();
    let track = TrackRef::video(editor.view.targets.video.unwrap_or(0));
    let Some(edge) = sequence
        .track(track)
        .and_then(|clips| nearest_transition_edge(clips, editor.playhead()))
    else {
        editor.notice("No clip on the targeted video track to add a transition to");
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
