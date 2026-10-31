use egui::RichText;
use timeline::{Sequence, SourceId, Ticks};

use crate::editor::Editor;

/// A sequence open in the timeline, and where its playhead was when last left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SequenceTab {
    /// `None` for the project's own sequence.
    pub(crate) nest: Option<SourceId>,
    pub(crate) playhead: Ticks,
}

/// A sequence's name in the project, or "Sequence" for the project's own.
pub(crate) fn sequence_name(project: &Sequence, nest: Option<SourceId>) -> &str {
    nest.and_then(|id| project.media_item(id))
        .map_or("Sequence", |item| item.name.as_str())
}

impl Editor {
    pub(crate) fn shown_sequence_name(&self) -> &str {
        sequence_name(self.timeline.project(), self.timeline.open_nest())
    }

    /// Gives the sequence now shown a tab, if it hasn't one yet.
    pub(crate) fn tab_shown_sequence(&mut self) {
        let nest = self.timeline.open_nest();
        if !self.sequence_tabs.iter().any(|tab| tab.nest == nest) {
            self.sequence_tabs.push(SequenceTab {
                nest,
                playhead: Ticks::ZERO,
            });
        }
    }

    fn switch_to_tab(&mut self, nest: Option<SourceId>) {
        let shown = self.timeline.open_nest();
        if nest == shown {
            return;
        }
        let playhead = self.playhead();
        if let Some(tab) = self.sequence_tabs.iter_mut().find(|tab| tab.nest == shown) {
            tab.playhead = playhead;
        }
        let Some(tab) = self
            .sequence_tabs
            .iter()
            .find(|tab| tab.nest == nest)
            .copied()
        else {
            return;
        };
        // A tab is reached directly, not through the nests above it.
        self.nest_trail.clear();
        if self.show_nest(nest) {
            self.seek_program(tab.playhead);
        }
    }

    fn close_tab(&mut self, nest: Option<SourceId>) {
        let Some(index) = self.sequence_tabs.iter().position(|tab| tab.nest == nest) else {
            return;
        };
        self.sequence_tabs.remove(index);
        if nest == self.timeline.open_nest() {
            // The project's sequence keeps the first tab, so one is always left before.
            let before = self.sequence_tabs.get(index.saturating_sub(1));
            if let Some(next) = before.map(|tab| tab.nest) {
                self.switch_to_tab(next);
            }
        }
    }

    /// Drops the tabs of sequences that are gone, as after undoing their creation.
    pub(crate) fn prune_sequence_tabs(&mut self) {
        let project = self.timeline.project();
        self.sequence_tabs.retain(|tab| {
            tab.nest.is_none_or(|id| {
                project
                    .media_item(id)
                    .is_some_and(|item| item.nest.is_some())
            })
        });
        self.tab_shown_sequence();
    }
}

/// A tab for each open sequence; clicking one shows it, its cross closes it. The
/// project's own sequence stays open.
pub(crate) fn tabs(editor: &mut Editor, ui: &mut egui::Ui) {
    if editor.sequence_tabs.len() < 2 {
        return;
    }
    let shown = editor.timeline.open_nest();
    let project = editor.timeline.project();
    let mut picked = None;
    let mut closed = None;
    ui.horizontal(|ui| {
        for tab in &editor.sequence_tabs {
            let name = sequence_name(project, tab.nest);
            if ui.selectable_label(tab.nest == shown, name).clicked() {
                picked = Some(tab.nest);
            }
            if tab.nest.is_some() {
                let cross = ui.small_button(RichText::new("×").weak());
                if cross.on_hover_text("Close this sequence").clicked() {
                    closed = Some(tab.nest);
                }
            }
            ui.separator();
        }
    });
    if let Some(nest) = closed {
        editor.close_tab(nest);
    } else if let Some(nest) = picked {
        editor.switch_to_tab(nest);
    }
}
