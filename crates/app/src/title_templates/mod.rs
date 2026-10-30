//! Ready-made titles to start from, and titles the user keeps to start from again.

mod browser;
mod catalog;
mod headings;
mod overlays;
mod previews;

pub(crate) use browser::{ui as browser_ui, GraphicsBrowser};

use egui::Key;
use project::TemplatesFile;
use timeline::{ClipId, Param, SetClipLook, Ticks, Timebase, TitleTemplate};

use catalog::{Builtin, Category, BUILTINS};

use crate::editor::Editor;
use crate::placing;
use crate::project_store::Setting;

pub(crate) struct TemplateDialog {
    clip: ClipId,
    name: String,
    opened: bool,
}

impl Editor {
    fn new_from_builtin(&mut self, builtin: &Builtin) {
        let sequence = self.timeline.sequence();
        let built = builtin.build(sequence.format.width, sequence.format.height);
        let length = built
            .seconds
            .and_then(|seconds| seconds_in(seconds, sequence.timebase));
        let clip = placing::place_generated(self, builtin.name.to_owned(), built.generator, length);
        if let (Some(clip), Some(position)) = (clip, built.position) {
            self.move_new_clip(clip, position);
        }
    }

    fn new_from_saved(&mut self, index: usize) {
        let Some(template) = self.title_templates.get(index).cloned() else {
            return;
        };
        let length = template.length_in(self.timeline.sequence().timebase);
        placing::place_generated(self, template.name, template.generator, length);
    }

    /// Places a just-made clip's picture `position` pixels from the frame's center.
    fn move_new_clip(&mut self, clip: ClipId, position: [i32; 2]) {
        let Some((_, placed)) = self.timeline.sequence().clip(clip) else {
            return;
        };
        let mut look = placed.look.clone();
        look.motion.position = position.map(|pixels| Param::new(pixels.saturating_mul(100)));
        self.apply(SetClipLook { clip, look });
    }

    /// Asks for a name to keep `clip`'s title or graphic under.
    pub(crate) fn name_title_template(&mut self, clip: ClipId) {
        let sequence = self.timeline.sequence();
        let keeps = sequence.clip(clip).is_some_and(|(_, clip)| {
            sequence
                .media
                .iter()
                .any(|item| item.id == clip.source && TitleTemplate::is_kept(item))
        });
        if !keeps {
            self.notice("Only titles and graphics can be kept as title templates");
            return;
        }
        self.template_dialog = Some(TemplateDialog {
            clip,
            name: format!("Template {:02}", self.title_templates.len() + 1),
            opened: true,
        });
    }

    fn delete_title_template(&mut self, index: usize) {
        if index < self.title_templates.len() {
            self.title_templates.remove(index);
            self.save_title_templates();
        }
    }

    pub(crate) fn load_title_templates(&mut self, text: &str) {
        match TemplatesFile::from_text(text) {
            Ok(file) => self.title_templates = file.templates,
            Err(error) => self.notice(format!("Cannot read title templates: {error}")),
        }
    }

    fn save_title_templates(&mut self) {
        let text = match TemplatesFile::new(self.title_templates.clone()).to_text() {
            Ok(text) => text,
            Err(error) => {
                self.notice(format!("Cannot save title templates: {error}"));
                return;
            }
        };
        if let Some(store) = &mut self.store {
            store.write_setting(Setting::TitleTemplates, text);
        }
    }

    /// Keeps `clip`'s title as `name`, replacing a template already called that.
    fn keep_title_template(&mut self, clip: ClipId, name: String) {
        let sequence = self.timeline.sequence();
        let Some((_, placed)) = sequence.clip(clip) else {
            return;
        };
        let length = placed.duration;
        let Some(template) =
            TitleTemplate::from_media(name.clone(), sequence, placed.source, length)
        else {
            return;
        };
        self.title_templates.retain(|kept| kept.name != name);
        self.title_templates.push(template);
        self.save_title_templates();
        self.notice(format!("Kept \u{201C}{name}\u{201D} under My Templates"));
    }
}

fn seconds_in(seconds: i64, timebase: Timebase) -> Option<Ticks> {
    Timebase::new(1, 1).map(|second| Ticks(seconds).rescale(second, timebase))
}

/// File › New › Title From Template: the built-in ones by kind, then the user's own,
/// which can be deleted from their context menus.
pub(crate) fn menu(ui: &mut egui::Ui, editor: &mut Editor) {
    ui.menu_button("Title From Template", |ui| {
        for category in Category::ALL {
            ui.menu_button(category.name(), |ui| {
                for builtin in BUILTINS.iter().filter(|b| b.category == category) {
                    if ui.button(builtin.name).clicked() {
                        editor.new_from_builtin(builtin);
                        ui.close();
                    }
                }
            });
        }
        ui.separator();
        ui.menu_button("My Templates", |ui| {
            if editor.title_templates.is_empty() {
                ui.weak("Keep a title with Save as Title Template… on its clip");
            }
            let mut deleted = None;
            for index in 0..editor.title_templates.len() {
                let response = ui.button(&editor.title_templates[index].name);
                if response.clicked() {
                    editor.new_from_saved(index);
                    ui.close();
                }
                response.context_menu(|ui| {
                    if ui.button("Delete").clicked() {
                        deleted = Some(index);
                    }
                });
            }
            if let Some(index) = deleted {
                editor.delete_title_template(index);
            }
        });
    });
}

pub(crate) fn dialog(editor: &mut Editor, ctx: &egui::Context) {
    let Some(dialog) = &mut editor.template_dialog else {
        return;
    };
    let mut done = None;
    crate::dialog::window(ctx, "Save as Title Template").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label("Name");
            let field = ui.text_edit_singleline(&mut dialog.name);
            if crate::dialog::opening(ui, &mut dialog.opened) {
                crate::dialog::focus_text(ui, &field, &dialog.name);
            }
        });
        ui.weak("Kept with how long the clip runs, under File › New › Title From Template.");
        crate::dialog::footer(ui, |ui| {
            if ui.button("Cancel").clicked() || ui.input(|input| input.key_pressed(Key::Escape)) {
                done = Some(false);
            }
            if ui.add(crate::dialog::primary("Save")).clicked()
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
    let clip = dialog.clip;
    editor.template_dialog = None;
    if keep && !name.is_empty() {
        editor.keep_title_template(clip, name);
    }
}
