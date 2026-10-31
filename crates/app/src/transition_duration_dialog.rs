//! Set Transition Duration: a double-clicked transition's length, typed in.

use egui::Key;
use timeline::{ClipId, Edge, SetTransition, Ticks, Timecode};

use crate::editor::Editor;

pub(crate) struct TransitionDurationDialog {
    clip: ClipId,
    edge: Edge,
    duration: Ticks,
    longest: Ticks,
}

impl Editor {
    pub(crate) fn open_transition_duration(&mut self, clip: ClipId, edge: Edge, longest: Ticks) {
        let transition = self
            .timeline
            .sequence()
            .clip(clip)
            .and_then(|(_, clip)| match edge {
                Edge::Head => clip.transition_in,
                Edge::Tail => clip.transition_out,
            });
        if let Some(transition) = transition {
            self.transition_duration_dialog = Some(TransitionDurationDialog {
                clip,
                edge,
                duration: transition.duration.min(longest),
                longest,
            });
        }
    }

    fn set_transition_duration(&mut self, dialog: &TransitionDurationDialog) {
        let sequence = self.timeline.sequence();
        let transition = sequence
            .clip(dialog.clip)
            .and_then(|(_, clip)| match dialog.edge {
                Edge::Head => clip.transition_in,
                Edge::Tail => clip.transition_out,
            });
        let Some(transition) = transition else {
            return;
        };
        if transition.duration == dialog.duration {
            return;
        }
        let mut changed = transition;
        changed.duration = dialog.duration;
        self.apply(SetTransition {
            clip: dialog.clip,
            edge: dialog.edge,
            transition: Some(changed),
        });
    }
}

pub(crate) fn dialog(editor: &mut Editor, ctx: &egui::Context) {
    let sequence = editor.timeline.sequence();
    let (timebase, rate, display) = (
        sequence.timebase,
        sequence.format.frame_rate,
        sequence.time_display,
    );
    let Some(dialog) = &mut editor.transition_duration_dialog else {
        return;
    };
    let mut done = None;
    crate::dialog::window(ctx, "Set Transition Duration").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label("Duration");
            let longest = rate.frame_at(dialog.longest, timebase).max(1);
            let mut frames = rate.frame_at(dialog.duration, timebase).clamp(1, longest);
            let field = crate::timecode_field::frames(&mut frames, (timebase, rate, display))
                .range(1..=longest);
            if ui.add(field).changed() {
                let duration = rate.frame_start(frames, timebase);
                dialog.duration = duration.min(dialog.longest);
            }
        });
        let longest = Timecode::shown(dialog.longest, timebase, rate, display);
        ui.weak(format!(
            "At most {longest}, as long as the clips it joins allow."
        ));
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
    let Some(set) = done else {
        return;
    };
    if let Some(dialog) = editor.transition_duration_dialog.take() {
        if set {
            editor.set_transition_duration(&dialog);
        }
    }
}
