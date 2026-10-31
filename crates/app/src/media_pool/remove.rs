use egui::{Context, Key};
use timeline::{ConsolidateDuplicates, Edit, RemoveMedia, RemoveUnused, SourceId};

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

    pub(crate) fn remove_unused(&mut self) {
        self.tidy_pool(RemoveUnused, "Every item is used in a sequence");
    }

    pub(crate) fn consolidate_duplicates(&mut self) {
        self.tidy_pool(ConsolidateDuplicates, "No two items are the same footage");
    }

    /// Applies an edit that takes items out of the project, and lets go of those it took
    /// wherever the editor still holds them.
    fn tidy_pool(&mut self, edit: impl Edit, untouched: &str) {
        let before = self.timeline.project().media.len();
        if !self.apply_to_pool(edit) {
            self.notice(untouched);
            return;
        }
        let project = self.timeline.project();
        let removed = before - project.media.len();
        let kept = |id: SourceId| project.media_item(id).is_some();
        self.pool.selected.retain(|&id| kept(id));
        if self
            .source
            .as_ref()
            .is_some_and(|shown| !kept(shown.source))
        {
            self.source = None;
        }
        let plural = if removed == 1 { "" } else { "s" };
        self.notice(format!("Took {removed} item{plural} out of the project"));
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
