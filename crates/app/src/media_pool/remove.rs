use egui::{Context, Key};
use timeline::{RemoveMedia, SourceId};

use crate::editor::Editor;

impl Editor {
    /// Removes an item from the project, first asking when that takes clips with it.
    pub(crate) fn remove_media(&mut self, source: SourceId) {
        let used = self
            .timeline
            .project()
            .all_clips()
            .any(|clip| clip.source == source);
        if used {
            self.pool.removing = Some(source);
        } else {
            self.remove_media_now(source);
        }
    }

    fn remove_media_now(&mut self, source: SourceId) {
        if self.source_shows(source) {
            self.source = None;
        }
        self.apply_to_pool(RemoveMedia { source });
    }
}

pub(crate) fn dialog(editor: &mut Editor, ctx: &Context) {
    let Some(source) = editor.pool.removing else {
        return;
    };
    let project = editor.timeline.project();
    let Some(item) = project.media_item(source) else {
        editor.pool.removing = None;
        return;
    };
    let clips = project
        .all_clips()
        .filter(|clip| clip.source == source)
        .count();
    let plural = if clips == 1 { "" } else { "s" };
    let mut remove = None;
    crate::dialog::window(ctx, "Remove from Project").show(ctx, |ui| {
        ui.label(format!(
            "“{}” is used by {clips} clip{plural} in the project's sequences.",
            item.name
        ));
        ui.label("Removing it removes those clips too.");
        crate::dialog::footer(ui, |ui| {
            if ui.button("Cancel").clicked() || ui.input(|input| input.key_pressed(Key::Escape)) {
                remove = Some(false);
            }
            if ui.add(crate::dialog::primary("Remove")).clicked()
                || ui.input(|input| input.key_pressed(Key::Enter))
            {
                remove = Some(true);
            }
        });
    });
    if let Some(remove) = remove {
        editor.pool.removing = None;
        if remove {
            editor.remove_media_now(source);
        }
    }
}
