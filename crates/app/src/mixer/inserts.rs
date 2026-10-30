use egui::RichText;
use timeline::{AudioEffect, Edit, SetMasterEffects, SetTrackEffects};

use super::{live, submixes, Change, Channel};
use crate::editor::Editor;

/// The strip whose inserts are open below the strips, and their settings while a
/// control is being dragged.
#[derive(Debug, Default)]
pub(super) struct Inserts {
    open: Option<Channel>,
    adjusting: Option<(Channel, Vec<AudioEffect>)>,
}

/// A strip's button for its inserts, naming how many there are.
pub(super) fn toggle(editor: &mut Editor, ui: &mut egui::Ui, channel: Channel, count: usize) {
    let inserts = &mut editor.mixer.inserts;
    let open = inserts.open == Some(channel);
    let label = match count {
        0 => "fx".to_owned(),
        count => format!("fx {count}"),
    };
    let tip = "Effects on everything this strip mixes, before its fader";
    if ui
        .selectable_label(open, label)
        .on_hover_text(tip)
        .clicked()
    {
        inserts.open = (!open).then_some(channel);
        inserts.adjusting = None;
    }
}

/// The open strip's inserts, as Premiere's mixer shows a selected effect's settings.
pub(super) fn panel(editor: &mut Editor, ui: &mut egui::Ui) {
    let Some(channel) = editor.mixer.inserts.open else {
        return;
    };
    let Some((name, current)) = owned(editor, channel) else {
        editor.mixer.inserts.open = None;
        return;
    };
    let mut effects = adjusting(editor, channel).unwrap_or(current);
    ui.separator();
    ui.label(RichText::new(format!("{name} Effects")).strong());
    let playhead = editor.playhead();
    let (change, seek, open_editor) =
        crate::effects::track_effect_controls(ui, &mut effects, playhead, &mut editor.vst);
    if let Some(key) = seek {
        editor.seek_program(key);
    }
    if let Some(change) = change {
        set_effects(editor, channel, effects, change);
    }
    if let Some(index) = open_editor {
        let holder = crate::vst::Holder::Channel(channel);
        crate::vst::open_editor(editor, ui.ctx(), holder, index);
    }
}

fn adjusting(editor: &Editor, channel: Channel) -> Option<Vec<AudioEffect>> {
    match &editor.mixer.inserts.adjusting {
        Some((adjusting, effects)) if *adjusting == channel => Some(effects.clone()),
        _ => None,
    }
}

/// A strip's inserts, as they are while being adjusted.
pub(crate) fn effects(editor: &Editor, channel: Channel) -> Option<Vec<AudioEffect>> {
    adjusting(editor, channel).or_else(|| owned(editor, channel).map(|(_, effects)| effects))
}

pub(crate) fn set_effects(
    editor: &mut Editor,
    channel: Channel,
    effects: Vec<AudioEffect>,
    change: Change,
) {
    editor.mixer.inserts.adjusting = (change == Change::Live).then(|| (channel, effects.clone()));
    match channel {
        Channel::Track(track) => settle(editor, change, SetTrackEffects { track, effects }),
        Channel::Submix(id) => {
            if let Some(edit) = submixes::changed(editor, id, |submix| submix.effects = effects) {
                settle(editor, change, edit);
            }
        }
        Channel::Master => settle(editor, change, SetMasterEffects { effects }),
    }
}

pub(super) fn settle(editor: &mut Editor, change: Change, edit: impl Edit + 'static) {
    match change {
        Change::Live => live(editor, &edit),
        Change::Commit => editor.apply_levels(edit),
    }
}

/// The strip's name and inserts.
fn owned(editor: &Editor, channel: Channel) -> Option<(String, Vec<AudioEffect>)> {
    match channel {
        Channel::Track(track) => {
            let owner = editor.timeline.sequence().track(track)?;
            let name = owner
                .name
                .as_ref()
                .map_or_else(|| track.to_string(), |name| format!("{track} {name}"));
            Some((name, owner.effects.clone()))
        }
        Channel::Submix(id) => {
            let submix = submixes::find(editor, id)?;
            Some((submix.name.clone(), submix.effects.clone()))
        }
        Channel::Master => Some((
            "Master".to_owned(),
            editor.timeline.sequence().master_effects.clone(),
        )),
    }
}
