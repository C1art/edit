use egui::Key;
use project::PresetsFile;
use timeline::{ClipId, EffectPreset};

use crate::editor::Editor;
use crate::project_store::Setting;

/// What a name typed in the preset dialog is for. Both are taken when the dialog opens,
/// so edits or deletions made while it is open can't redirect the name.
enum Naming {
    New(EffectPreset),
    /// The preset called this.
    Rename(String),
}

pub(crate) struct PresetDialog {
    naming: Naming,
    name: String,
    opened: bool,
}

impl Editor {
    /// Asks for a name to keep `clip`'s effects under.
    pub(crate) fn name_new_preset(&mut self, clip: ClipId) {
        let Some(preset) = EffectPreset::from_clip(String::new(), self.timeline.sequence(), clip)
        else {
            self.notice("Add effects to the clip before saving them as a preset");
            return;
        };
        self.preset_dialog = Some(PresetDialog {
            naming: Naming::New(preset),
            name: format!("Preset {:02}", self.presets.len() + 1),
            opened: true,
        });
    }

    pub(crate) fn rename_preset(&mut self, index: usize) {
        if let Some(preset) = self.presets.get(index) {
            self.preset_dialog = Some(PresetDialog {
                naming: Naming::Rename(preset.name.clone()),
                name: preset.name.clone(),
                opened: true,
            });
        }
    }

    pub(crate) fn delete_preset(&mut self, index: usize) {
        if index < self.presets.len() {
            self.presets.remove(index);
            self.save_presets();
        }
    }

    pub(crate) fn load_presets(&mut self, text: &str) {
        match PresetsFile::from_text(text) {
            Ok(file) => {
                // Presets kept before the file was read would otherwise be lost, and the
                // file saved with only them.
                let early = std::mem::replace(&mut self.presets, file.presets);
                let changed = !early.is_empty();
                for preset in early {
                    match self
                        .presets
                        .iter_mut()
                        .find(|kept| kept.name == preset.name)
                    {
                        Some(kept) => *kept = preset,
                        None => self.presets.push(preset),
                    }
                }
                if changed {
                    self.save_presets();
                }
            }
            Err(error) => self.unreadable_setting(Setting::Presets, error),
        }
    }

    fn save_presets(&mut self) {
        let text = match PresetsFile::new(self.presets.clone()).to_text() {
            Ok(text) => text,
            Err(error) => {
                self.notice(format!("Cannot save effect presets: {error}"));
                return;
            }
        };
        if let Some(store) = &mut self.store {
            store.write_setting(Setting::Presets, text);
        }
    }

    /// Keeps the dialog's name, replacing a preset already called that.
    fn finish_naming(&mut self, naming: Naming, name: String) {
        let index = match naming {
            Naming::New(mut preset) => {
                preset.name.clone_from(&name);
                self.presets.push(preset);
                self.presets.len() - 1
            }
            Naming::Rename(old) => {
                let Some(index) = self.presets.iter().position(|preset| preset.name == old) else {
                    return;
                };
                self.presets[index].name.clone_from(&name);
                index
            }
        };
        let mut position = 0;
        self.presets.retain(|preset| {
            let keep = position == index || preset.name != name;
            position += 1;
            keep
        });
        self.save_presets();
    }
}

pub(crate) fn dialog(editor: &mut Editor, ctx: &egui::Context) {
    let Some(dialog) = &mut editor.preset_dialog else {
        return;
    };
    let title = match &dialog.naming {
        Naming::New(_) => "Save Preset",
        Naming::Rename(_) => "Rename Preset",
    };
    let mut done = None;
    crate::dialog::window(ctx, title).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label("Name");
            let field = ui.text_edit_singleline(&mut dialog.name);
            if crate::dialog::opening(ui, &mut dialog.opened) {
                crate::dialog::focus_text(ui, &field, &dialog.name);
            }
        });
        crate::dialog::footer(ui, |ui| {
            if ui.button("Cancel").clicked() || ui.input(|input| input.key_pressed(Key::Escape)) {
                done = Some(false);
            }
            if ui.add(crate::dialog::primary("OK")).clicked()
                || ui.input(|input| input.key_pressed(Key::Enter))
            {
                done = Some(true);
            }
        });
    });
    let Some(keep) = done else {
        return;
    };
    let name = dialog.name.trim().to_owned();
    let Some(dialog) = editor.preset_dialog.take() else {
        return;
    };
    if keep && !name.is_empty() {
        editor.finish_naming(dialog.naming, name);
    }
}
