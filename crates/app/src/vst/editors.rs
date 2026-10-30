//! Plugins' own editor windows, with what is done in them brought into the timeline as
//! edits like any other.

use std::collections::HashMap;
use std::sync::Arc;

use plugins::{ClassId, EditorEvent, EditorSetup, EditorWindow, ParamId};
use timeline::{AudioEffect, ClipId, PluginEffect, PluginState, Ticks};

use super::{Described, PluginKey};
use crate::editor::Editor;
use crate::mixer::{Change, Channel};

/// What holds the effects an editor's plugin is one of.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Holder {
    Clip(ClipId),
    Channel(Channel),
}

pub(super) struct OpenEditor {
    holder: Holder,
    index: usize,
    key: PluginKey,
    window: EditorWindow,
    /// The value each control in the window shows, as the model rounds it, so a change
    /// made elsewhere is passed on once and the window's own changes aren't echoed.
    shown: HashMap<u32, f32>,
}

/// Opens the editor of the plugin effect at `index` of `holder`'s effects, unless open.
pub(crate) fn open_editor(editor: &mut Editor, ctx: &egui::Context, holder: Holder, index: usize) {
    let open = &editor.vst.editors;
    if open
        .iter()
        .any(|open| open.holder == holder && open.index == index)
    {
        return;
    }
    let Some(effect) = plugin_at(editor, holder, index) else {
        return;
    };
    let time = time(editor, holder);
    let shown: HashMap<u32, f32> = effect
        .params
        .iter()
        .map(|param| (param.id, param.value.value_at(time)))
        .collect();
    let ctx = ctx.clone();
    let setup = EditorSetup {
        path: effect.path.clone(),
        class_id: ClassId(effect.class_id),
        title: effect.name.clone(),
        state: effect.state.bytes().to_vec(),
        params: shown
            .iter()
            .map(|(&id, &value)| (ParamId(id), f64::from(value)))
            .collect(),
        notify: Arc::new(move || ctx.request_repaint()),
    };
    match EditorWindow::open(setup) {
        Ok(window) => editor.vst.editors.push(OpenEditor {
            holder,
            index,
            key: (effect.path, effect.class_id),
            window,
            shown,
        }),
        Err(error) => editor.notice(format!("Can't open {}: {error}", effect.name)),
    }
}

/// Applies what was done in each open editor, closes those whose effect is gone and
/// passes changes made elsewhere on to the windows.
pub(crate) fn tend_editors(editor: &mut Editor) {
    let mut open = std::mem::take(&mut editor.vst.editors);
    open.retain_mut(|window| tend(editor, window));
    // An editor opened while these were tended is kept too.
    open.append(&mut editor.vst.editors);
    editor.vst.editors = open;
}

/// False once the window is closed or its effect is gone.
fn tend(editor: &mut Editor, open: &mut OpenEditor) -> bool {
    let Some(mut effect) = plugin_at(editor, open.holder, open.index)
        .filter(|effect| (effect.path.clone(), effect.class_id) == open.key)
    else {
        return false;
    };
    let time = time(editor, open.holder);
    let mut change = None;
    let mut alive = true;
    for event in open.window.events() {
        match event {
            EditorEvent::Begin(_) | EditorEvent::End(_) => {}
            // Heard live; the state that follows each finished gesture commits it.
            EditorEvent::Change(id, value) => {
                let shown = set_value(&mut effect, id, value, time);
                open.shown.insert(id.0, shown);
                change = change.max(Some(Change::Live));
            }
            EditorEvent::State(state) => {
                effect.state = PluginState::new(state);
                change = Some(Change::Commit);
            }
            EditorEvent::Reloaded { state, values } => {
                effect.state = PluginState::new(state);
                let defaults = defaults(editor, &open.key);
                for (id, value) in values {
                    let animated = effect.param(id.0).is_some_and(|param| param.is_animated());
                    let default = defaults.get(&id.0).copied();
                    let changed = effect.param(id.0).is_some()
                        || default.is_some_and(|default| round(default) != round(value));
                    if changed && !animated {
                        let shown = set_value(&mut effect, id, value, time);
                        open.shown.insert(id.0, shown);
                    }
                }
                change = Some(Change::Commit);
            }
            EditorEvent::Closed(state) => {
                if let Some(state) = state.filter(|state| state != effect.state.bytes()) {
                    effect.state = PluginState::new(state);
                    change = Some(Change::Commit);
                }
                alive = false;
            }
            EditorEvent::Failed(error) => {
                editor.notice(format!("Can't open {}'s editor: {error}", effect.name));
                alive = false;
            }
        }
    }
    if let Some(change) = change {
        set_plugin(editor, open.holder, open.index, effect.clone(), change);
    }
    if alive {
        show_changes(editor, open, &effect, time);
    }
    alive
}

/// Moves the window's controls whose values were changed elsewhere, such as in Effect
/// Controls, by undo or by keyframes as the playhead moves.
fn show_changes(editor: &mut Editor, open: &mut OpenEditor, effect: &PluginEffect, time: Ticks) {
    for param in &effect.params {
        let value = param.value.value_at(time);
        if open.shown.insert(param.id, value) != Some(value) {
            open.window
                .set_parameter(ParamId(param.id), f64::from(value));
        }
    }
    let dropped: Vec<u32> = open
        .shown
        .keys()
        .copied()
        .filter(|&id| effect.param(id).is_none())
        .collect();
    if dropped.is_empty() {
        return;
    }
    let defaults = defaults(editor, &open.key);
    for id in dropped {
        open.shown.remove(&id);
        if let Some(&default) = defaults.get(&id) {
            open.window.set_parameter(ParamId(id), default);
        }
    }
}

/// Sets parameter `id` at `time`; the value as the model holds it.
fn set_value(effect: &mut PluginEffect, id: ParamId, value: f64, time: Ticks) -> f32 {
    let value = value as f32;
    let param = effect.param_mut(id.0, value);
    param.set_at(time, value);
    param.value_at(time)
}

fn round(value: f64) -> i64 {
    (value * 100.0).round() as i64
}

fn defaults(editor: &mut Editor, key: &PluginKey) -> HashMap<u32, f64> {
    match editor.vst.describe(key) {
        Some(Described::Params(params)) => params
            .iter()
            .map(|info| (info.id.0, info.default_normalized))
            .collect(),
        _ => HashMap::new(),
    }
}

fn plugin_at(editor: &Editor, holder: Holder, index: usize) -> Option<PluginEffect> {
    match effects(editor, holder)?.into_iter().nth(index)? {
        AudioEffect::Plugin(effect) => Some(*effect),
        _ => None,
    }
}

fn effects(editor: &Editor, holder: Holder) -> Option<Vec<AudioEffect>> {
    match holder {
        Holder::Clip(id) => {
            let (_, clip) = editor.timeline.sequence().clip(id)?;
            Some(crate::effects::clip_audio_effects(editor, clip))
        }
        Holder::Channel(channel) => crate::mixer::channel_effects(editor, channel),
    }
}

fn set_plugin(
    editor: &mut Editor,
    holder: Holder,
    index: usize,
    effect: PluginEffect,
    change: Change,
) {
    let Some(mut effects) = effects(editor, holder) else {
        return;
    };
    let Some(slot) = effects.get_mut(index) else {
        return;
    };
    *slot = AudioEffect::Plugin(Box::new(effect));
    match holder {
        Holder::Clip(id) => crate::effects::set_clip_audio_effects(editor, id, effects, change),
        Holder::Channel(channel) => {
            crate::mixer::set_channel_effects(editor, channel, effects, change);
        }
    }
}

/// The time keyframes are read at: the clip's own for a clip, the sequence's for a strip.
fn time(editor: &Editor, holder: Holder) -> Ticks {
    let clip = match holder {
        Holder::Clip(id) => editor.timeline.sequence().clip(id),
        Holder::Channel(_) => None,
    };
    match clip {
        Some((_, clip)) => crate::effects::keyframe_time(editor, clip),
        None => editor.playhead(),
    }
}
