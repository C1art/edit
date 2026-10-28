use egui::Key;
use timeline::{ClipId, SetClipSpeed, Speed, Ticks, Timecode};

use crate::editor::Editor;

/// The Speed/Duration dialog's state while it's open.
pub struct SpeedPrompt {
    clips: Vec<ClipId>,
    /// The source the clips play, which stays the same whatever the speed.
    source: Ticks,
    percent: f64,
    duration: Ticks,
    /// Whether the duration was typed last, so it's kept exactly instead of rounded
    /// through the percentage.
    duration_typed: bool,
    ripple: bool,
}

/// Like `150%` or `33.33%`.
pub fn percent_text(speed: Speed) -> String {
    let text = format!("{:.2}", speed.percent());
    let text = text.trim_end_matches('0').trim_end_matches('.');
    format!("{text}%")
}

impl Editor {
    /// Opens Speed/Duration for the selected clips.
    pub(crate) fn speed_duration(&mut self) {
        let sequence = self.timeline.sequence();
        let Some((_, clip)) = self.view.selection.iter().find_map(|&id| sequence.clip(id)) else {
            self.notice("Select a clip to change its speed");
            return;
        };
        self.speed_prompt = Some(SpeedPrompt {
            clips: self.view.selection.clone(),
            source: clip.source_out() - clip.source_in,
            percent: clip.speed.percent(),
            duration: clip.duration,
            duration_typed: false,
            ripple: false,
        });
    }
}

pub fn dialog(editor: &mut Editor, ctx: &egui::Context) {
    let sequence = editor.timeline.sequence();
    let (timebase, rate) = (sequence.timebase, sequence.format.frame_rate);
    let Some(prompt) = &mut editor.speed_prompt else {
        return;
    };
    let mut choice = None;
    egui::Window::new("Clip Speed / Duration")
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            egui::Grid::new("speed_fields")
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("Speed");
                    let percent = ui.add(
                        egui::DragValue::new(&mut prompt.percent)
                            .range(1.0..=10_000.0)
                            .speed(1.0)
                            .max_decimals(2)
                            .suffix("%"),
                    );
                    if percent.changed() {
                        prompt.duration_typed = false;
                        if let Some(speed) = typed_speed(prompt.percent) {
                            prompt.duration = speed.sequence_span(prompt.source).max(Ticks(1));
                        }
                    }
                    ui.end_row();
                    ui.label("Duration");
                    let mut frames = rate.frame_at(prompt.duration, timebase).max(1);
                    let duration = ui.add(
                        egui::DragValue::new(&mut frames)
                            .range(1..=i64::MAX)
                            .speed(0.5)
                            .custom_formatter(|frames, _| {
                                let ticks = rate.frame_start(frames as i64, timebase);
                                Timecode::new(ticks, timebase, rate).to_string()
                            }),
                    );
                    if duration.changed() {
                        prompt.duration_typed = true;
                        prompt.duration = rate.frame_start(frames, timebase);
                        if let Some(speed) = Speed::stretch(prompt.source, prompt.duration) {
                            prompt.percent = speed.percent();
                        }
                    }
                    ui.end_row();
                });
            ui.checkbox(&mut prompt.ripple, "Ripple Edit, Shifting Trailing Clips");
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() || ui.input(|input| input.key_pressed(Key::Enter)) {
                    choice = Some(true);
                }
                if ui.button("Cancel").clicked() || ui.input(|input| input.key_pressed(Key::Escape))
                {
                    choice = Some(false);
                }
            });
        });
    let Some(apply) = choice else {
        return;
    };
    let Some(prompt) = editor.speed_prompt.take() else {
        return;
    };
    if !apply {
        return;
    }
    let speed = if prompt.duration_typed {
        Speed::stretch(prompt.source, prompt.duration)
    } else {
        typed_speed(prompt.percent)
    };
    match speed {
        Some(speed) => {
            editor.apply(SetClipSpeed {
                clips: prompt.clips,
                speed,
                ripple: prompt.ripple,
            });
        }
        None => editor.notice("Cannot change speed: the duration must be positive"),
    }
}

/// The percentage as typed, kept to hundredths of a percent.
fn typed_speed(percent: f64) -> Option<Speed> {
    let points = (percent * 100.0).round();
    if !(1.0..=u64::MAX as f64).contains(&points) {
        return None;
    }
    Speed::from_basis_points(points as u64)
}
