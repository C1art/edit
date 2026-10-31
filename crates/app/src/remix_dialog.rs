use std::time::Duration;

use egui::{Key, ProgressBar};
use playback::{RemixEvent, RemixJob};
use timeline::{ClipId, RemixClip, Ticks, Timebase, TrackKind};

use crate::editor::Editor;

const MILLISECONDS: Timebase = match Timebase::new(1, 1_000) {
    Some(timebase) => timebase,
    None => unreachable!(),
};
/// Long enough to hide a seam's click, short enough not to smear a beat.
const CROSSFADE_MS: i64 = 20;

/// Remix: the music clip being cut or looped to a new length, and the job listening
/// to it while it plans the cuts.
pub(crate) struct RemixDialog {
    clip: ClipId,
    target: Ticks,
    listening: Option<Listening>,
}

struct Listening {
    job: RemixJob,
    progress: f32,
}

impl Editor {
    /// Opens Remix for the first selected audio clip.
    pub(crate) fn open_remix(&mut self) {
        let sequence = self.timeline.sequence();
        let music = sequence
            .clips_of(&sequence.with_links(&self.view.selection))
            .into_iter()
            .find(|(track, _)| track.kind == TrackKind::Audio)
            .map(|(_, clip)| clip);
        match music {
            Some(clip) if clip.rate().is_normal() && !clip.reverse && clip.remap.is_none() => {
                self.remix_dialog = Some(RemixDialog {
                    clip: clip.id,
                    target: clip.duration,
                    listening: None,
                });
            }
            Some(_) => self.notice("Remix needs a clip at normal speed"),
            None => self.notice("Select a music clip first"),
        }
    }
}

pub(crate) fn dialog(editor: &mut Editor, ctx: &egui::Context) {
    let sequence = editor.timeline.sequence();
    let (timebase, rate, display) = (
        sequence.timebase,
        sequence.format.frame_rate,
        sequence.time_display,
    );
    let Some(dialog) = &mut editor.remix_dialog else {
        return;
    };
    let mut planned = None;
    let mut warnings = Vec::new();
    if let Some(listening) = &mut dialog.listening {
        for event in listening.job.poll() {
            match event {
                RemixEvent::Progress(progress) => listening.progress = progress,
                RemixEvent::Warning(warning) => warnings.push(warning),
                RemixEvent::Planned(pieces) => planned = Some(pieces),
            }
        }
        ctx.request_repaint_after(Duration::from_millis(100));
    }
    let mut apply = false;
    let mut cancelled = false;
    crate::dialog::window(ctx, "Remix").show(ctx, |ui| {
        let listening = dialog.listening.as_ref();
        ui.add_enabled_ui(listening.is_none(), |ui| {
            ui.horizontal(|ui| {
                ui.label("Target Duration");
                let mut frames = rate.frame_at(dialog.target, timebase).max(1);
                let field = crate::timecode_field::frames(&mut frames, (timebase, rate, display))
                    .range(1..=i64::MAX);
                if ui.add(field).changed() {
                    dialog.target = rate.frame_start(frames, timebase);
                }
            });
        });
        ui.label("The music is cut or looped where it sounds alike, keeping its start and end.");
        if let Some(listening) = listening {
            ui.add(
                ProgressBar::new(listening.progress)
                    .show_percentage()
                    .text("Analyzing…"),
            );
        }
        crate::dialog::footer(ui, |ui| {
            cancelled =
                ui.button("Cancel").clicked() || ui.input(|input| input.key_pressed(Key::Escape));
            apply = listening.is_none()
                && (ui.add(crate::dialog::primary("OK")).clicked()
                    || ui.input(|input| input.key_pressed(Key::Enter)));
        });
    });
    for warning in warnings {
        editor.notice(format!("Heard as silence: {warning}"));
    }
    if cancelled {
        editor.remix_dialog = None;
    } else if let Some(pieces) = planned {
        remix(editor, pieces);
    } else if apply {
        start(editor);
    }
}

fn start(editor: &mut Editor) {
    let Some(dialog) = editor.remix_dialog.as_mut() else {
        return;
    };
    match RemixJob::start(editor.timeline.sequence(), dialog.clip, dialog.target) {
        Ok(Some(job)) => {
            dialog.listening = Some(Listening { job, progress: 0.0 });
        }
        Ok(None) => editor.remix_dialog = None,
        Err(error) => editor.notice(format!("Cannot remix: {error}")),
    }
}

fn remix(editor: &mut Editor, pieces: Vec<std::ops::Range<Ticks>>) {
    let Some(dialog) = editor.remix_dialog.take() else {
        return;
    };
    let timebase = editor.timeline.sequence().timebase;
    let length = pieces
        .iter()
        .fold(Ticks(0), |length, piece| length + (piece.end - piece.start));
    let crossfade = Ticks(CROSSFADE_MS).rescale(MILLISECONDS, timebase);
    let remixed = editor.apply(RemixClip {
        clip: dialog.clip,
        pieces,
        crossfade,
    });
    if remixed {
        let missed = editor
            .timeline
            .sequence()
            .timecode(Ticks((length - dialog.target).0.abs()));
        let side = if length > dialog.target {
            "longer"
        } else {
            "shorter"
        };
        if length != dialog.target {
            editor.notice(format!(
                "Remixed to the nearest seamless length, {missed} {side} than asked"
            ));
        }
    }
}
