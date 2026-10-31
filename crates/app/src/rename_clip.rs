use egui::Key;
use timeline::{ClipId, RenameClips};

use crate::editor::Editor;

/// The name being typed for the selected clips.
pub(crate) struct ClipRename {
    clips: Vec<ClipId>,
    name: String,
    opened: bool,
}

impl Editor {
    pub(crate) fn rename_clips(&mut self) {
        let sequence = self.timeline.sequence();
        let clips = self.view.selection.clone();
        let Some((_, first)) = clips.first().and_then(|&id| sequence.clip(id)) else {
            self.notice("Select the clips to rename first");
            return;
        };
        let item = sequence.media_item(first.source);
        let name = first
            .name
            .clone()
            .or_else(|| item.map(|item| item.name.clone()))
            .unwrap_or_default();
        self.clip_rename = Some(ClipRename {
            clips,
            name,
            opened: true,
        });
    }
}

pub(crate) fn dialog(editor: &mut Editor, ctx: &egui::Context) {
    let Some(rename) = &mut editor.clip_rename else {
        return;
    };
    let mut choice = None;
    crate::dialog::window(ctx, "Rename Clip").show(ctx, |ui| {
        ui.label("Clip Name");
        let field = ui.text_edit_singleline(&mut rename.name);
        if crate::dialog::opening(ui, &mut rename.opened) {
            crate::dialog::focus_text(ui, &field, &rename.name);
        }
        ui.weak("Leave empty to show the media's name");
        crate::dialog::footer(ui, |ui| {
            if ui.button("Cancel").clicked() || ui.input(|input| input.key_pressed(Key::Escape)) {
                choice = Some(false);
            }
            if ui.add(crate::dialog::primary("OK")).clicked()
                || ui.input(|input| input.key_pressed(Key::Enter))
            {
                choice = Some(true);
            }
        });
    });
    let Some(apply) = choice else {
        return;
    };
    if let Some(ClipRename { clips, name, .. }) = editor.clip_rename.take().filter(|_| apply) {
        editor.apply(RenameClips { clips, name });
    }
}
