use timeline::{AddNest, ClipId, NestClips, SourceId, Ticks};

use crate::editor::Editor;

/// A sequence left to open a nest inside it, and where its playhead was, so returning
/// puts the playhead back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Level {
    pub nest: Option<SourceId>,
    pub playhead: Ticks,
}

impl Editor {
    /// Moves the selected clips into a new nested sequence that takes their place.
    pub(crate) fn nest_selection(&mut self) {
        if self.view.selection.is_empty() {
            self.notice("Select the clips to nest");
            return;
        }
        let name = self.next_sequence_name("Nested Sequence");
        let clips = self.view.selection.clone();
        if self.apply(NestClips { clips, name }) {
            let sequence = self.timeline.sequence();
            let nest = sequence.media.last().map(|item| item.id);
            self.view.selection = sequence
                .tracks()
                .flat_map(|(_, track)| &track.clips)
                .filter(|clip| Some(clip.source) == nest)
                .map(|clip| clip.id)
                .collect();
        }
    }

    /// Adds an empty nested sequence and opens it.
    pub(crate) fn new_sequence(&mut self) {
        let name = self.next_sequence_name("Sequence");
        if self.apply(AddNest { name }) {
            if let Some(id) = self.timeline.sequence().media.last().map(|item| item.id) {
                self.enter_nest(id, Ticks::ZERO);
            }
        }
    }

    fn next_sequence_name(&self, kind: &str) -> String {
        let count = self
            .timeline
            .project()
            .media
            .iter()
            .filter(|item| item.nest.is_some())
            .count();
        format!("{kind} {:02}", count + 1)
    }

    /// Opens what a clip shows: a nest in the timeline, with the playhead on the frame
    /// the clip shows under it, or other media in the source monitor.
    pub(crate) fn open_clip(&mut self, id: ClipId) {
        let sequence = self.timeline.sequence();
        let Some((_, clip)) = sequence.clip(id) else {
            return;
        };
        let Some(item) = sequence.media_item(clip.source) else {
            return;
        };
        if item.nest.is_none() {
            self.open_source(item.id);
            return;
        }
        let playhead = self.playhead();
        let inside = if clip.covers(playhead) {
            clip.source_time(playhead)
        } else {
            clip.source_in
        };
        self.enter_nest(item.id, inside);
    }

    pub(crate) fn enter_nest(&mut self, nest: SourceId, playhead: Ticks) {
        let level = Level {
            nest: self.timeline.open_nest(),
            playhead: self.playhead(),
        };
        if self.show_nest(Some(nest)) {
            self.nest_trail.push(level);
            self.seek_program(playhead);
        }
    }

    /// Returns to the sequence `depth` levels from the project's own.
    pub(crate) fn leave_nest(&mut self, depth: usize) {
        let Some(&level) = self.nest_trail.get(depth) else {
            return;
        };
        self.nest_trail.truncate(depth);
        if self.show_nest(level.nest) {
            self.seek_program(level.playhead);
        }
    }

    fn show_nest(&mut self, nest: Option<SourceId>) -> bool {
        // Opening a nest changes what is shown, not the project.
        let clean = !self.is_dirty();
        if !self.timeline.set_open_nest(nest) {
            return false;
        }
        if clean {
            self.saved_revision = self.timeline.revision();
        }
        self.view.selection.clear();
        true
    }

    /// Forgets the way back once the open nest is gone, as after undoing its creation.
    pub(crate) fn prune_nest_trail(&mut self) {
        if self.timeline.open_nest().is_none() {
            self.nest_trail.clear();
        }
    }
}

/// The sequences leading to the open nest, each clickable to return to it.
pub fn breadcrumbs(editor: &mut Editor, ui: &mut egui::Ui) {
    if editor.nest_trail.is_empty() {
        return;
    }
    let project = editor.timeline.project();
    let name = |nest: Option<SourceId>| {
        nest.and_then(|id| project.media_item(id))
            .map_or("Sequence".to_owned(), |item| item.name.clone())
    };
    let mut back = None;
    ui.horizontal(|ui| {
        for (depth, level) in editor.nest_trail.iter().enumerate() {
            if ui.link(name(level.nest)).clicked() {
                back = Some(depth);
            }
            ui.label("›");
        }
        ui.strong(name(editor.timeline.open_nest()));
    });
    if let Some(depth) = back {
        editor.leave_nest(depth);
    }
}
