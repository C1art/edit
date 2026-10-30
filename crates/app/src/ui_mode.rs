//! Simple mode keeps to what a short edit or a first project needs; Advanced shows all
//! of it. Shortcuts work in both, so nothing learned in Simple stops working.

use crate::editor::Editor;
use crate::effects::SideTab;
use crate::effects_panel::EffectItem;
use crate::panels::MediaTab;
use crate::theme;
use crate::timeline_view::Tool;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum UiMode {
    Simple,
    #[default]
    Advanced,
}

impl UiMode {
    pub(crate) const ALL: [Self; 2] = [Self::Simple, Self::Advanced];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Simple => "Simple",
            Self::Advanced => "Advanced",
        }
    }

    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Simple => "simple",
            Self::Advanced => "advanced",
        }
    }

    pub(crate) fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.key() == key)
    }

    fn hint(self) -> &'static str {
        match self {
            Self::Simple => "Only the basics: cut, trim, titles, transitions and export",
            Self::Advanced => "Every panel, tool, effect and command",
        }
    }
}

pub(crate) const SIMPLE_MEDIA_TABS: [MediaTab; 3] =
    [MediaTab::Project, MediaTab::Effects, MediaTab::Graphics];
pub(crate) const SIMPLE_SIDE_TABS: [SideTab; 4] = [
    SideTab::EffectControls,
    SideTab::LumetriColor,
    SideTab::EssentialSound,
    SideTab::Text,
];

const SIMPLE_VIDEO_EFFECTS: [&str; 12] = [
    "Gaussian Blur",
    "Black & White",
    "Brightness & Contrast",
    "Tint",
    "Sharpen",
    "Vignette",
    "Horizontal Flip",
    "Vertical Flip",
    "Mirror",
    "Mosaic",
    "Drop Shadow",
    "Warp Stabilizer",
];

const SIMPLE_TRANSITIONS: [&str; 8] = [
    "Cross Dissolve",
    "Dip to Black",
    "Dip to White",
    "Push",
    "Slide",
    "Wipe",
    "Cross Zoom",
    "Whip",
];

const SIMPLE_AUDIO_EFFECTS: [&str; 6] = [
    "DeNoise",
    "DeHum",
    "Bass",
    "Treble",
    "Studio Reverb",
    "Hard Limiter",
];

/// Whether Simple mode's Effects panel lists `item`; presets always show, as they are
/// the quickest way to a finished look.
pub(crate) fn simple_effect(item: &EffectItem) -> bool {
    match item {
        EffectItem::Video(effect) => SIMPLE_VIDEO_EFFECTS.contains(&effect.name()),
        EffectItem::Audio(effect) => SIMPLE_AUDIO_EFFECTS.contains(&effect.name()),
        EffectItem::Transition(timeline::TrackKind::Video, kind) => {
            SIMPLE_TRANSITIONS.contains(&kind.name())
        }
        EffectItem::Transition(timeline::TrackKind::Audio, kind) => {
            kind.audio_name() == "Constant Power" || kind.audio_name() == "Dip to Silence"
        }
        EffectItem::Preset(_) | EffectItem::Motion(_) => true,
    }
}

/// The tool groups Simple mode shows, and any group holding the current tool, so that
/// a tool picked by its shortcut is still seen to be on.
pub(crate) fn tool_groups(mode: UiMode, current: Tool) -> Vec<&'static [Tool]> {
    Tool::GROUPS
        .into_iter()
        .filter(|group| {
            mode == UiMode::Advanced
                || group.contains(&current)
                || group.iter().any(|&tool| simple_tool(tool))
        })
        .collect()
}

fn simple_tool(tool: Tool) -> bool {
    matches!(
        tool,
        Tool::Selection | Tool::Razor | Tool::Hand | Tool::Zoom | Tool::Type | Tool::Shape(_)
    )
}

impl Editor {
    pub(crate) fn simple(&self) -> bool {
        self.ui_mode == UiMode::Simple
    }

    pub(crate) fn set_ui_mode(&mut self, mode: UiMode) {
        self.ui_mode = mode;
        if mode == UiMode::Simple && !simple_tool(self.view.tool) {
            self.view.tool = Tool::Selection;
        }
        self.keep_tabs_in_mode();
    }

    /// Moves off a tab the mode hides, as a workspace may have picked one.
    pub(crate) fn keep_tabs_in_mode(&mut self) {
        if !self.simple() {
            return;
        }
        if !SIMPLE_MEDIA_TABS.contains(&self.media_tab) {
            self.media_tab = MediaTab::Project;
        }
        if !SIMPLE_SIDE_TABS.contains(&self.side_tab) {
            self.side_tab = SideTab::EffectControls;
        }
    }
}

/// Simple and Advanced side by side at the end of the menu bar.
pub(crate) fn switch(editor: &mut Editor, ui: &mut egui::Ui) {
    let mut chosen = None;
    theme::segmented(ui, |ui| {
        // The menu bar's end is laid out right to left.
        for mode in UiMode::ALL.into_iter().rev() {
            let label = ui.selectable_label(editor.ui_mode == mode, mode.name());
            if label.on_hover_text(mode.hint()).clicked() {
                chosen = Some(mode);
            }
        }
    });
    if let Some(mode) = chosen.filter(|&mode| mode != editor.ui_mode) {
        editor.set_ui_mode(mode);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use timeline::{AudioEffect, TransitionKind, VideoEffect};

    #[test]
    fn every_simple_effect_is_in_the_catalogs() {
        let video: Vec<_> = VideoEffect::catalog().iter().map(|e| e.name()).collect();
        let audio: Vec<_> = AudioEffect::catalog().iter().map(|e| e.name()).collect();
        let transitions: Vec<_> = TransitionKind::VIDEO.iter().map(|k| k.name()).collect();
        for name in SIMPLE_VIDEO_EFFECTS {
            assert!(video.contains(&name), "{name}");
        }
        for name in SIMPLE_AUDIO_EFFECTS {
            assert!(audio.contains(&name), "{name}");
        }
        for name in SIMPLE_TRANSITIONS {
            assert!(transitions.contains(&name), "{name}");
        }
    }
}
