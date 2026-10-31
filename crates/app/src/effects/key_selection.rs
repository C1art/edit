use egui::Id;
use timeline::{ClipId, Keyframe, Param, Ticks};

use crate::editor::Editor;

/// What an edit key asks of the selected keyframes, done by each lane as it is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyAction {
    Copy,
    Paste,
    Delete,
    SelectAll,
}

/// The keyframes picked in Effect Controls' lanes, each under its parameter's id.
#[derive(Debug, Default)]
pub(crate) struct KeySelection {
    /// A lane was the last thing clicked, so the edit keys act on keyframes.
    focused: bool,
    /// A lane was pressed this frame, which keeps the focus.
    pressed: bool,
    clip: Option<ClipId>,
    selected: Vec<(Id, Ticks)>,
    copied: Vec<(Id, Keyframe)>,
    action: Option<KeyAction>,
    /// The property last changed or clicked, which Add/Remove Keyframe keys.
    current: Option<Id>,
    /// Add/Remove Keyframe was asked for, done by the current property as it's drawn.
    toggle_current: bool,
}

impl KeySelection {
    pub(super) fn is_selected(&self, param: Id, time: Ticks) -> bool {
        self.selected.contains(&(param, time))
    }

    pub(super) fn selected_in(&self, param: Id) -> Vec<Ticks> {
        let mine = self.selected.iter().filter(|&&(id, _)| id == param);
        mine.map(|&(_, time)| time).collect()
    }

    pub(super) fn press(&mut self) {
        self.focused = true;
        self.pressed = true;
    }

    /// Picks the keyframe at `time`, alone or, `adding`, toggled among the others.
    pub(super) fn pick(&mut self, param: Id, time: Ticks, adding: bool) {
        let entry = (param, time);
        if !adding {
            self.selected = vec![entry];
        } else if let Some(index) = self.selected.iter().position(|&kept| kept == entry) {
            self.selected.swap_remove(index);
        } else {
            self.selected.push(entry);
        }
    }

    pub(super) fn touch(&mut self, param: Id) {
        self.current = Some(param);
    }

    /// Whether `param` is to add or remove its keyframe at the playhead now.
    pub(super) fn toggles(&self, param: Id) -> bool {
        self.toggle_current && self.current == Some(param)
    }

    pub(super) fn clear(&mut self) {
        self.selected.clear();
    }

    /// Follows the selected keyframes of `param` that were moved by `by`.
    pub(super) fn shift(&mut self, param: Id, by: Ticks) {
        for (id, time) in &mut self.selected {
            if *id == param {
                *time = *time + by;
            }
        }
    }

    /// Does the pending action to `param`, with the playhead at source `time`; returns
    /// whether `param` changed.
    pub(super) fn act(&mut self, id: Id, param: &mut Param, time: Ticks) -> bool {
        match self.action {
            None => false,
            Some(KeyAction::Copy) => {
                let keys = param
                    .keys
                    .iter()
                    .filter(|key| self.is_selected(id, key.time));
                let keys: Vec<_> = keys.map(|&key| (id, key)).collect();
                self.copied.extend(keys);
                false
            }
            Some(KeyAction::Paste) => {
                let Some(first) = self.copied.iter().map(|(_, key)| key.time).min() else {
                    return false;
                };
                let mine = self.copied.iter().filter(|&&(copied, _)| copied == id);
                let pasted: Vec<Keyframe> = mine
                    .map(|&(_, key)| Keyframe {
                        time: key.time - first + time,
                        ..key
                    })
                    .collect();
                if pasted.is_empty() {
                    return false;
                }
                // Pasting into a parameter that wasn't animated keeps its value outside
                // the pasted keys, as one keyframe at a time would.
                for key in pasted {
                    param.put_key(key);
                    self.selected.push((id, key.time));
                }
                true
            }
            Some(KeyAction::Delete) => {
                let selected = self.selected_in(id);
                let kept: Vec<Ticks> = selected
                    .into_iter()
                    .filter(|&key| param.has_key_at(key))
                    .collect();
                for &key in &kept {
                    param.toggle_key(key);
                }
                self.selected.retain(|&(selected, _)| selected != id);
                !kept.is_empty()
            }
            Some(KeyAction::SelectAll) => {
                let all = param.keys.iter().map(|key| (id, key.time));
                let new: Vec<_> = all.filter(|entry| !self.selected.contains(entry)).collect();
                self.selected.extend(new);
                false
            }
        }
    }

    /// Ends a frame of Effect Controls showing `clip`: the action is done, and a press
    /// anywhere but a lane gives the edit keys back to the timeline.
    pub(super) fn end_frame(&mut self, ui: &egui::Ui, clip: ClipId) {
        if self.clip != Some(clip) {
            self.clip = Some(clip);
            self.selected.clear();
        }
        if ui.input(|input| input.pointer.any_pressed()) && !self.pressed {
            self.focused = false;
        }
        self.pressed = false;
        self.action = None;
        self.toggle_current = false;
    }
}

impl Editor {
    /// Adds or removes a keyframe at the playhead on the property last touched in
    /// Effect Controls, starting its animation if it had none.
    pub(crate) fn toggle_focused_keyframe(&mut self) {
        let shown = self.side_tab == super::SideTab::EffectControls;
        if !shown || self.effects.keys.current.is_none() {
            self.notice("Click a property in Effect Controls to add or remove its keyframes");
            return;
        }
        self.effects.keys.toggle_current = true;
        self.ctx.request_repaint();
    }

    /// Hands `action` to the keyframes when Effect Controls' lanes have the focus;
    /// returns whether they took it.
    pub(crate) fn act_on_keyframes(&mut self, action: KeyAction) -> bool {
        let keys = &mut self.effects.keys;
        let shown = self.side_tab == super::SideTab::EffectControls;
        let takes = match action {
            KeyAction::Copy | KeyAction::Delete => !keys.selected.is_empty(),
            KeyAction::Paste => !keys.copied.is_empty(),
            KeyAction::SelectAll => true,
        };
        if !(shown && keys.focused && takes) {
            return false;
        }
        if action == KeyAction::Paste {
            keys.selected.clear();
        }
        if action == KeyAction::Copy {
            keys.copied.clear();
            // Paste only arrives as an event while the system clipboard holds text.
            let count = keys.selected.len();
            self.ctx.copy_text(format!("{count} keyframes"));
        }
        keys.action = Some(action);
        true
    }
}
