//! Several selected clips adjusted at once: their position, scale, rotation and opacity,
//! or their volume, raised or lowered together in one step. Each clip moves by the same
//! amount, so it keeps its own differences and animation.

use std::ops::RangeInclusive;

use egui::RichText;
use timeline::{AdjustClips, Clip, ClipId, Gain, Look, Param, TrackKind};

use super::{hear_live, keyframe_time, show_live};
use crate::editor::Editor;
use crate::mixer::{response_state, Change};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Property {
    PositionX,
    PositionY,
    Scale,
    Rotation,
    Opacity,
    Volume,
}

const PICTURE: [Property; 5] = [
    Property::PositionX,
    Property::PositionY,
    Property::Scale,
    Property::Rotation,
    Property::Opacity,
];

impl Property {
    fn label(self) -> &'static str {
        match self {
            Self::PositionX => "Position X",
            Self::PositionY => "Position Y",
            Self::Scale => "Scale",
            Self::Rotation => "Rotation",
            Self::Opacity => "Opacity",
            Self::Volume => "Volume",
        }
    }

    fn range(self) -> RangeInclusive<f32> {
        match self {
            Self::PositionX | Self::PositionY => -10_000.0..=10_000.0,
            Self::Scale => 0.0..=1_000.0,
            Self::Rotation => -3_600.0..=3_600.0,
            Self::Opacity => 0.0..=100.0,
            Self::Volume => Gain::SILENT.db()..=Gain::MAX.db(),
        }
    }

    fn suffix(self) -> &'static str {
        match self {
            Self::PositionX | Self::PositionY => "",
            Self::Scale | Self::Opacity => "%",
            Self::Rotation => "°",
            Self::Volume => " dB",
        }
    }

    fn param(self, clip: &Clip) -> &Param {
        let motion = &clip.look.motion;
        match self {
            Self::PositionX => &motion.position[0],
            Self::PositionY => &motion.position[1],
            Self::Scale => &motion.scale,
            Self::Rotation => &motion.rotation,
            Self::Opacity => &clip.look.opacity,
            Self::Volume => &clip.volume,
        }
    }

    /// `look` raised by `by`; a non-uniform scale grows in both directions.
    fn offset_look(self, look: &Look, by: f32) -> Look {
        let mut look = look.clone();
        let range = self.range();
        let motion = &mut look.motion;
        match self {
            Self::PositionX => motion.position[0].offset(by, range),
            Self::PositionY => motion.position[1].offset(by, range),
            Self::Scale => {
                if !motion.uniform_scale {
                    motion.scale_width.offset(by, range.clone());
                }
                motion.scale.offset(by, range);
            }
            Self::Rotation => motion.rotation.offset(by, range),
            Self::Opacity => look.opacity.offset(by, range),
            Self::Volume => {}
        }
        look
    }
}

/// The section shown above the first clip's own controls while more than one video or
/// audio clip is selected.
pub(super) fn controls(editor: &mut Editor, ui: &mut egui::Ui, selected: &[ClipId]) {
    if selected.len() < 2 {
        return;
    }
    let sequence = editor.timeline.sequence();
    let of = |kind: TrackKind| -> Vec<Clip> {
        selected
            .iter()
            .filter_map(|&id| sequence.clip(id))
            .filter(|(track, _)| track.kind == kind)
            .map(|(_, clip)| clip.clone())
            .collect()
    };
    let (pictures, sounds) = (of(TrackKind::Video), of(TrackKind::Audio));
    if pictures.len() < 2 && sounds.len() < 2 {
        return;
    }
    let count = pictures.len().max(sounds.len());
    egui::CollapsingHeader::new(format!("All {count} Selected Clips"))
        .default_open(true)
        .show(ui, |ui| {
            ui.label(RichText::new("Changes move every selected clip by the same amount").weak());
            egui::Grid::new("adjust together")
                .num_columns(3)
                .show(ui, |ui| {
                    if pictures.len() > 1 {
                        for property in PICTURE {
                            row(editor, ui, property, &pictures);
                        }
                    }
                    if sounds.len() > 1 {
                        row(editor, ui, Property::Volume, &sounds);
                    }
                });
        });
}

fn row(editor: &mut Editor, ui: &mut egui::Ui, property: Property, clips: &[Clip]) {
    let shown: Vec<f32> = clips
        .iter()
        .map(|clip| property.param(clip).value_at(keyframe_time(editor, clip)))
        .collect();
    let Some(&reference) = shown.first() else {
        return;
    };
    let mixed = shown.iter().any(|value| (value - reference).abs() >= 0.005);
    let mut value = match editor.effects.together {
        Some((dragging, value)) if dragging == property => value,
        _ => reference,
    };
    ui.label(property.label());
    let response = ui.add(
        egui::DragValue::new(&mut value)
            .range(property.range())
            .suffix(property.suffix())
            .max_decimals(2),
    );
    if mixed {
        ui.label(RichText::new("Mixed").weak()).on_hover_text(
            "The clips differ here; each keeps its difference, and the first shows its value",
        );
    } else {
        ui.label("");
    }
    ui.end_row();
    let Some(change) = response_state(&response, None) else {
        return;
    };
    editor.effects.together = match change {
        Change::Live => Some((property, value)),
        Change::Commit => None,
    };
    let by = value - reference;
    let edit = if property == Property::Volume {
        AdjustClips {
            looks: Vec::new(),
            volumes: clips
                .iter()
                .map(|clip| {
                    let mut volume = clip.volume.clone();
                    volume.offset(by, property.range());
                    (clip.id, volume)
                })
                .collect(),
        }
    } else {
        AdjustClips {
            looks: clips
                .iter()
                .map(|clip| (clip.id, property.offset_look(&clip.look, by)))
                .collect(),
            volumes: Vec::new(),
        }
    };
    match change {
        Change::Live if property == Property::Volume => hear_live(editor, &edit),
        Change::Live => show_live(editor, &edit),
        Change::Commit if property == Property::Volume => editor.apply_levels(edit),
        Change::Commit => {
            editor.apply(edit);
        }
    }
}
