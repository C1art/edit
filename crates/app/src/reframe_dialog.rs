use egui::Key;
use timeline::{ReframeMotion, ReframeSequence, VideoEffect};

use crate::editor::Editor;

/// The shapes Premiere's Auto Reframe Sequence offers, as width to height.
const ASPECTS: [(&str, u32, u32); 4] = [
    ("Square 1:1", 1, 1),
    ("Vertical 4:5", 4, 5),
    ("Vertical 9:16", 9, 16),
    ("Horizontal 16:9", 16, 9),
];

/// The target shape and motion preset for Auto Reframe Sequence.
pub(crate) struct ReframeDialog {
    aspect: usize,
    motion: ReframeMotion,
}

impl Editor {
    pub(crate) fn open_reframe_dialog(&mut self) {
        if self.timeline.open_nest().is_some() {
            self.notice("A nested sequence uses the project's frame size");
            return;
        }
        self.reframe_dialog = Some(ReframeDialog {
            aspect: 2,
            motion: ReframeMotion::default(),
        });
    }

    /// Queues analysis of every clip whose Auto Reframe hasn't found its subject.
    fn analyze_unframed(&mut self) {
        let sequence = self.timeline.sequence();
        let unframed: Vec<_> = sequence
            .video
            .iter()
            .flat_map(|track| &track.clips)
            .filter(|clip| {
                clip.look.effects.iter().any(|effect| {
                    matches!(effect, VideoEffect::AutoReframe(reframe) if reframe.subject.is_none())
                })
            })
            .map(|clip| clip.id)
            .collect();
        for clip in unframed {
            self.analyze_reframe(clip);
        }
    }
}

/// The frame of shape `width:height` whose shorter side matches the shorter side of
/// `frame`, with even sides for subsampled chroma.
fn reshaped(frame: (u32, u32), (width, height): (u32, u32)) -> (u32, u32) {
    let short = u64::from(frame.0.min(frame.1));
    let even = |side: u64| u32::try_from(side + side % 2).unwrap_or(u32::MAX);
    if width <= height {
        (
            even(short),
            even(short * u64::from(height) / u64::from(width)),
        )
    } else {
        (
            even(short * u64::from(width) / u64::from(height)),
            even(short),
        )
    }
}

pub(crate) fn dialog(editor: &mut Editor, ctx: &egui::Context) {
    let Some(settings) = &mut editor.reframe_dialog else {
        return;
    };
    let format = editor.timeline.sequence().format;
    let mut done = false;
    let mut cancelled = false;
    egui::Window::new("Auto Reframe Sequence")
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            egui::Grid::new("auto reframe")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    ui.label("Target aspect ratio");
                    egui::ComboBox::from_id_salt("reframe aspect")
                        .selected_text(ASPECTS[settings.aspect].0)
                        .show_ui(ui, |ui| {
                            for (index, (name, ..)) in ASPECTS.iter().enumerate() {
                                ui.selectable_value(&mut settings.aspect, index, *name);
                            }
                        });
                    ui.end_row();
                    ui.label("Motion tracking");
                    egui::ComboBox::from_id_salt("reframe motion")
                        .selected_text(settings.motion.name())
                        .show_ui(ui, |ui| {
                            for motion in ReframeMotion::ALL {
                                ui.selectable_value(&mut settings.motion, motion, motion.name());
                            }
                        });
                    ui.end_row();
                });
            let (_, width, height) = ASPECTS[settings.aspect];
            let (width, height) = reshaped((format.width, format.height), (width, height));
            ui.weak(format!(
                "The sequence becomes {width} × {height}, and each clip of footage follows \
                 its subject."
            ));
            ui.horizontal(|ui| {
                done = ui.button("Create").clicked()
                    || ui.input(|input| input.key_pressed(Key::Enter));
                cancelled = ui.button("Cancel").clicked()
                    || ui.input(|input| input.key_pressed(Key::Escape));
            });
        });
    if !(done || cancelled) {
        return;
    }
    let Some(settings) = editor.reframe_dialog.take() else {
        return;
    };
    if !done {
        return;
    }
    let (_, width, height) = ASPECTS[settings.aspect];
    let (width, height) = reshaped((format.width, format.height), (width, height));
    let edit = ReframeSequence {
        width,
        height,
        motion: settings.motion,
    };
    if editor.apply(edit) {
        editor.analyze_unframed();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reshaping_keeps_the_shorter_side() {
        let hd = (1920, 1080);
        let sizes: Vec<_> = ASPECTS
            .iter()
            .map(|&(_, width, height)| reshaped(hd, (width, height)))
            .collect();
        assert_eq!(
            sizes,
            [(1080, 1080), (1080, 1350), (1080, 1920), (1920, 1080)]
        );
        assert_eq!(reshaped((1080, 1920), (16, 9)), (1920, 1080));
    }
}
