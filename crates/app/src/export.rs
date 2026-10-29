use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::{ComboBox, DragValue, Key, ProgressBar, Slider};
use media::{AudioCodec, RateControl, VideoCodec};
use playback::{ExportEvent, ExportJob, ExportSettings};
use timeline::Sequence;

use crate::editor::Editor;
use crate::export_format::{ExportFormat, Format, PRESETS};
use crate::path_dialog::FolderUse;
use crate::project_files::PathPurpose;

/// Frame heights offered besides the sequence's own.
const HEIGHTS: [u32; 6] = [2160, 1440, 1080, 720, 540, 360];
const SAMPLE_RATES: [u32; 2] = [44_100, 48_000];
const AUDIO_KBPS: [u32; 4] = [128, 192, 256, 320];

struct Running {
    job: ExportJob,
    path: PathBuf,
    frames_done: i64,
    frames_total: i64,
}

/// What part of the sequence to export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Span {
    Sequence,
    InToOut,
}

/// The export dialog: settings while idle, progress while a job runs.
pub struct ExportDialog {
    path: String,
    format: ExportFormat,
    span: Span,
    running: Option<Running>,
}

/// Opens the dialog, suggesting a file next to the first imported media.
pub fn open(editor: &mut Editor) {
    if editor.export.is_some() {
        return;
    }
    let sequence = editor.timeline.sequence();
    let span = if sequence.marks.is_set() {
        Span::InToOut
    } else {
        Span::Sequence
    };
    let folder = editor
        .browse_folders
        .get(&FolderUse::Output)
        .cloned()
        .or_else(|| {
            let first = sequence.media.first()?;
            first.path.parent().map(PathBuf::from)
        })
        .unwrap_or_default();
    let format = ExportFormat::default();
    let file = format!("Sequence.{}", format.format.extension());
    editor.export = Some(ExportDialog {
        path: folder.join(file).display().to_string(),
        format,
        span,
        running: None,
    });
}

impl ExportDialog {
    pub fn output(&self) -> PathBuf {
        PathBuf::from(self.path.trim())
    }

    /// Writes to `path`, in the file type the chosen format makes.
    pub fn set_output(&mut self, path: &Path) {
        self.path = path
            .with_extension(self.format.format.extension())
            .display()
            .to_string();
    }
}

pub fn dialog(editor: &mut Editor, ctx: &egui::Context) {
    poll(editor);
    let sequence = editor.timeline.sequence();
    let marked = sequence.marks.is_set();
    let sequence_size = (sequence.format.width, sequence.format.height);
    // Escape in the file browser closes only that.
    let browsing = editor.path_prompt.is_some();
    let Some(dialog) = &mut editor.export else {
        return;
    };
    let mut start = false;
    let mut queue = false;
    let mut close = false;
    let mut browse = false;
    crate::dialog::window(ctx, "Export Media").show(ctx, |ui| match &dialog.running {
        Some(running) => {
            ui.label(format!("Exporting to {}", running.path.display()));
            let fraction = if running.frames_total > 0 {
                running.frames_done as f32 / running.frames_total as f32
            } else {
                0.0
            };
            ui.add(
                ProgressBar::new(fraction)
                    .desired_width(420.0)
                    .text(format!(
                        "frame {} of {}",
                        running.frames_done, running.frames_total
                    )),
            );
            crate::dialog::footer(ui, |ui| {
                if ui.button("Cancel").clicked() {
                    running.job.cancel();
                }
            });
            ctx.request_repaint();
        }
        None => {
            let kind = dialog.format.format;
            settings(ui, &mut dialog.format, sequence_size);
            if dialog.format.format != kind {
                dialog.set_output(&dialog.output());
            }
            ui.label("Output file:");
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut dialog.path).desired_width(340.0));
                browse = ui.button("Browse…").clicked();
            });
            ui.horizontal(|ui| {
                ui.label("Source range:");
                ui.radio_value(&mut dialog.span, Span::Sequence, "Entire sequence");
                ui.add_enabled_ui(marked, |ui| {
                    ui.radio_value(&mut dialog.span, Span::InToOut, "Sequence in/out")
                        .on_disabled_hover_text("Mark in or out on the program first");
                });
            });
            ui.label(summary(&dialog.format, sequence_size));
            crate::dialog::footer(ui, |ui| {
                close = ui.button("Close").clicked()
                    || (!browsing && ui.input(|input| input.key_pressed(Key::Escape)));
                queue = ui
                    .button("Queue")
                    .on_hover_text("Add to the export queue and keep editing")
                    .clicked();
                start = ui.add(crate::dialog::primary("Export")).clicked();
            });
        }
    });
    if browse {
        editor.prompt_path(PathPurpose::ExportMedia);
    } else if close {
        editor.export = None;
    } else if start {
        start_job(editor);
    } else if queue {
        queue_job(editor);
    }
}

/// The sequence and settings the dialog would export, after telling the user what's
/// wrong if it can't.
fn prepare(editor: &mut Editor) -> Option<(Arc<Sequence>, ExportSettings)> {
    let dialog = editor.export.as_ref()?;
    let path = PathBuf::from(dialog.path.trim().trim_matches('"'));
    let sequence = editor.timeline.sequence();
    let sequence_size = (sequence.format.width, sequence.format.height);
    let (width, height) = dialog.format.size(sequence_size);
    let range = (dialog.span == Span::InToOut && sequence.marks.is_set())
        .then(|| sequence.marks.range(sequence.duration()));
    let problem = if path.as_os_str().is_empty() {
        Some("Choose an output file first".to_owned())
    } else if sequence.duration() == timeline::Ticks::ZERO {
        Some("The sequence is empty".to_owned())
    } else if dialog.format.format.has_video()
        && !(width.is_multiple_of(2) && height.is_multiple_of(2))
    {
        Some(format!("The frame size must be even, not {width}×{height}"))
    } else if range.as_ref().is_some_and(|range| range.is_empty()) {
        Some("The in mark is past the end of the sequence".to_owned())
    } else {
        None
    };
    if let Some(problem) = problem {
        editor.notice(format!("Cannot export: {problem}"));
        return None;
    }
    let settings = ExportSettings {
        path,
        video: dialog.format.video(sequence_size),
        audio: dialog.format.audio(),
        range,
    };
    Some((Arc::new(sequence.clone()), settings))
}

fn start_job(editor: &mut Editor) {
    let Some((sequence, settings)) = prepare(editor) else {
        return;
    };
    let Some(render_state) = &editor.render_state else {
        editor.notice("Cannot export: there is no GPU renderer");
        return;
    };
    let path = settings.path.clone();
    let started = ExportJob::start(
        sequence,
        render_state.device.clone(),
        render_state.queue.clone(),
        settings,
    );
    match started {
        Ok(job) => {
            if let Some(dialog) = &mut editor.export {
                dialog.running = Some(Running {
                    job,
                    path,
                    frames_done: 0,
                    frames_total: 0,
                });
            }
        }
        Err(error) => editor.notice(format!("Cannot export: {error}")),
    }
}

/// Hands the export to the queue and closes the dialog, so another can be set up.
fn queue_job(editor: &mut Editor) {
    let Some((sequence, settings)) = prepare(editor) else {
        return;
    };
    let summary = editor.export.as_ref().map_or_else(String::new, |dialog| {
        summary(
            &dialog.format,
            (sequence.format.width, sequence.format.height),
        )
    });
    editor.export_queue.add(sequence, settings, summary);
    editor.export_queue.open = true;
    editor.export = None;
}

fn settings(ui: &mut egui::Ui, format: &mut ExportFormat, sequence_size: (u32, u32)) {
    egui::Grid::new("export settings")
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            ui.label("Preset:");
            let current = format.preset_name();
            ComboBox::from_id_salt("export preset")
                .width(260.0)
                .selected_text(current.unwrap_or("Custom"))
                .show_ui(ui, |ui| {
                    for preset in &PRESETS {
                        if ui
                            .selectable_label(current == Some(preset.name), preset.name)
                            .clicked()
                        {
                            *format = preset.format;
                        }
                    }
                });
            ui.end_row();
            ui.label("Format:");
            ComboBox::from_id_salt("export format")
                .width(260.0)
                .selected_text(format.format.name())
                .show_ui(ui, |ui| {
                    for kind in Format::ALL {
                        ui.selectable_value(&mut format.format, kind, kind.name());
                    }
                });
            ui.end_row();
            if format.format.has_video() {
                let codec = format.format.video_codec();
                let rate_controlled = codec.is_some_and(VideoCodec::is_rate_controlled);
                video_rows(ui, format, sequence_size, rate_controlled);
            }
            if format.has_audio() {
                ui.label("Sample rate:");
                ui.horizontal(|ui| {
                    for rate in SAMPLE_RATES {
                        let text = format!("{} kHz", rate as f32 / 1_000.0);
                        ui.selectable_value(&mut format.sample_rate, rate, text);
                    }
                });
                ui.end_row();
                if format.audio_is_lossy() {
                    ui.label("Audio bitrate:");
                    ui.horizontal(|ui| {
                        for kbps in AUDIO_KBPS {
                            let text = format!("{kbps} kbps");
                            ui.selectable_value(&mut format.audio_kbps, kbps, text);
                        }
                    });
                    ui.end_row();
                }
            }
        });
}

fn video_rows(
    ui: &mut egui::Ui,
    format: &mut ExportFormat,
    sequence_size: (u32, u32),
    rate_controlled: bool,
) {
    ui.label("Frame size:");
    let size_text = |height: Option<u32>| {
        let (width, height) = ExportFormat { height, ..*format }.size(sequence_size);
        if (width, height) == sequence_size {
            format!("{width}×{height} (sequence)")
        } else {
            format!("{width}×{height}")
        }
    };
    ComboBox::from_id_salt("export size")
        .width(260.0)
        .selected_text(size_text(format.height))
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut format.height, None, size_text(None));
            for height in HEIGHTS
                .into_iter()
                .filter(|&height| height != sequence_size.1)
            {
                ui.selectable_value(&mut format.height, Some(height), size_text(Some(height)));
            }
        });
    ui.end_row();
    if rate_controlled {
        ui.label("Bitrate:");
        ui.horizontal(|ui| {
            let by_quality = matches!(format.rate_control, RateControl::Quality(_));
            if ui.radio(by_quality, "Quality").clicked() && !by_quality {
                format.rate_control = RateControl::Quality(18);
            }
            if ui.radio(!by_quality, "Target").clicked() && by_quality {
                format.rate_control = RateControl::Bitrate(16_000_000);
            }
            match &mut format.rate_control {
                RateControl::Quality(crf) => {
                    ui.add(Slider::new(crf, 0..=40).text("CRF, lower is better"));
                }
                RateControl::Bitrate(bits) => {
                    let mut mbps = *bits as f32 / 1e6;
                    let drag = DragValue::new(&mut mbps)
                        .range(1.0..=400.0)
                        .speed(0.2)
                        .max_decimals(1)
                        .suffix(" Mbps");
                    if ui.add(drag).changed() {
                        *bits = (mbps * 1e6).round() as u32;
                    }
                }
            }
        });
        ui.end_row();
    }
    if format.format.audio_codec().is_some() {
        ui.label("Audio:");
        ui.checkbox(&mut format.with_audio, "Export audio");
        ui.end_row();
    }
}

fn summary(format: &ExportFormat, sequence_size: (u32, u32)) -> String {
    let audio = format.audio().map(|audio| {
        let codec = audio_codec_name(audio.codec);
        if audio.codec.is_lossy() {
            format!("{codec} {} kbps", format.audio_kbps)
        } else {
            codec.to_owned()
        }
    });
    match (format.video(sequence_size), audio) {
        (Some(video), Some(audio)) => format!(
            "{} at {}×{}, with {audio} sound.",
            format.format.name(),
            video.size.0,
            video.size.1
        ),
        (Some(video), None) => format!(
            "{} at {}×{}, silent.",
            format.format.name(),
            video.size.0,
            video.size.1
        ),
        (None, Some(audio)) => format!("Sound only, {audio}."),
        (None, None) => String::new(),
    }
}

fn audio_codec_name(codec: AudioCodec) -> &'static str {
    match codec {
        AudioCodec::Aac => "AAC",
        AudioCodec::Mp3 => "MP3",
        AudioCodec::Flac => "FLAC",
        AudioCodec::Pcm16 => "16-bit PCM",
        AudioCodec::Pcm24 => "24-bit PCM",
    }
}

fn poll(editor: &mut Editor) {
    let Some(running) = editor
        .export
        .as_mut()
        .and_then(|dialog| dialog.running.as_mut())
    else {
        return;
    };
    let mut notices = Vec::new();
    let mut ended = false;
    for event in running.job.poll() {
        match event {
            ExportEvent::Progress {
                frames_done,
                frames_total,
            } => {
                running.frames_done = frames_done;
                running.frames_total = frames_total;
            }
            ExportEvent::Warning(error) => notices.push(format!("Export: {error}")),
            ExportEvent::Finished => {
                notices.push(format!("Exported {}", running.path.display()));
                ended = true;
            }
            ExportEvent::Cancelled => {
                notices.push("Export cancelled".to_owned());
                ended = true;
            }
            ExportEvent::Failed(error) => {
                notices.push(format!("Export failed: {error}"));
                ended = true;
            }
        }
    }
    if ended {
        if let Some(dialog) = &mut editor.export {
            dialog.running = None;
        }
    }
    for notice in notices {
        editor.notice(notice);
    }
}
