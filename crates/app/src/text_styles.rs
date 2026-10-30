//! Text styles the user keeps across projects, named from a title's look in Effect
//! Controls and given to other titles from there.

use std::sync::Arc;

use egui::Key;
use project::TextStylesFile;
use timeline::{TextStyle, Title};

use crate::editor::Editor;
use crate::effects::StyleRequest;
use crate::project_store::Setting;

pub(crate) struct StyleDialog {
    look: Title,
    name: String,
    opened: bool,
}

impl Editor {
    pub(crate) fn text_style_request(&mut self, request: StyleRequest) {
        match request {
            StyleRequest::Save(look) => {
                self.text_style_dialog = Some(StyleDialog {
                    look: *look,
                    name: format!("Style {:02}", self.text_styles.len() + 1),
                    opened: true,
                });
            }
            StyleRequest::Delete(name) => {
                Arc::make_mut(&mut self.text_styles).retain(|style| style.name != name);
                self.save_text_styles();
            }
        }
    }

    pub(crate) fn load_text_styles(&mut self, text: &str) {
        match TextStylesFile::from_text(text) {
            Ok(file) => self.text_styles = Arc::new(file.styles),
            Err(error) => self.notice(format!("Cannot read text styles: {error}")),
        }
    }

    fn save_text_styles(&mut self) {
        let text = match TextStylesFile::new(self.text_styles.to_vec()).to_text() {
            Ok(text) => text,
            Err(error) => {
                self.notice(format!("Cannot save text styles: {error}"));
                return;
            }
        };
        if let Some(store) = &mut self.store {
            store.write_setting(Setting::TextStyles, text);
        }
    }

    /// Keeps `look` as `name`, replacing a style already called that, so a style can be
    /// changed by saving over it.
    fn keep_text_style(&mut self, look: &Title, name: String) {
        let styles = Arc::make_mut(&mut self.text_styles);
        let style = TextStyle::from_title(name.clone(), look);
        match styles.iter_mut().find(|kept| kept.name == name) {
            Some(kept) => *kept = style,
            None => styles.push(style),
        }
        self.save_text_styles();
        self.notice(format!("Kept the text style \u{201C}{name}\u{201D}"));
    }
}

pub(crate) fn dialog(editor: &mut Editor, ctx: &egui::Context) {
    let Some(dialog) = &mut editor.text_style_dialog else {
        return;
    };
    let mut done = None;
    crate::dialog::window(ctx, "Save Text Style").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label("Name");
            let field = ui.text_edit_singleline(&mut dialog.name);
            if crate::dialog::opening(ui, &mut dialog.opened) {
                crate::dialog::focus_text(ui, &field, &dialog.name);
            }
        });
        ui.weak("Font, size, fill, stroke, shadow and background, for any title or text layer.");
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
    let Some(dialog) = editor.text_style_dialog.take() else {
        return;
    };
    let name = dialog.name.trim().to_owned();
    if keep && !name.is_empty() {
        editor.keep_text_style(&dialog.look, name);
    }
}
