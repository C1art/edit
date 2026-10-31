use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::thread;

use crossbeam::channel::{self, Receiver, Sender};
use egui::{Grid, RichText};
use media::{MediaInfo, StreamInfo, StreamKind};
use timeline::{
    ClipId, FrameRate, MediaItem, MediaLog, SetMediaLog, SourceId, Ticks, TimeDisplay, Timebase,
    Timecode,
};

use crate::editor::Editor;
use crate::error::AppError;

type Probed = Result<(MediaInfo, u64), AppError>;

/// Narrower than this, a property's name goes above its value.
const SIDE_BY_SIDE_WIDTH: f32 = 240.0;

/// What a file's headers say, read once per file off the UI thread.
enum Probe {
    Reading,
    Read { info: MediaInfo, bytes: u64 },
    Failed(String),
}

/// Which selection the panel describes: the one that changed last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Follow {
    #[default]
    Project,
    Timeline,
}

pub(crate) struct MetadataPanel {
    probes: HashMap<PathBuf, Probe>,
    sender: Sender<(PathBuf, Probed)>,
    receiver: Receiver<(PathBuf, Probed)>,
    follow: Follow,
    seen_item: Option<SourceId>,
    seen_clip: Option<ClipId>,
    /// The log being typed into, kept apart until a field is left.
    editing: Option<(SourceId, MediaLog)>,
}

impl Default for MetadataPanel {
    fn default() -> Self {
        let (sender, receiver) = channel::unbounded();
        Self {
            probes: HashMap::new(),
            sender,
            receiver,
            follow: Follow::default(),
            seen_item: None,
            seen_clip: None,
            editing: None,
        }
    }
}

/// How many channels a file's sound has, as far as reading its headers has got.
pub(crate) enum Channels {
    Reading,
    Read(u16),
    Failed(String),
}

impl MetadataPanel {
    pub(crate) fn poll(&mut self) {
        for (path, probed) in self.receiver.try_iter() {
            let probe = match probed {
                Ok((info, bytes)) => Probe::Read { info, bytes },
                Err(error) => Probe::Failed(error.to_string()),
            };
            self.probes.insert(path, probe);
        }
    }

    fn probe(&mut self, path: &Path) -> &Probe {
        self.probes.entry(path.to_owned()).or_insert_with(|| {
            let (sender, path) = (self.sender.clone(), path.to_owned());
            let spawned = thread::Builder::new()
                .name("metadata-probe".to_owned())
                .spawn(move || {
                    let probed = (|| {
                        let info = media::probe(&path)?;
                        Ok((info, std::fs::metadata(&path)?.len()))
                    })();
                    let _ = sender.send((path, probed));
                });
            match spawned {
                Ok(_) => Probe::Reading,
                Err(error) => Probe::Failed(error.to_string()),
            }
        })
    }

    /// The channels of the file's first sound stream, reading its headers if not yet read.
    pub(crate) fn audio_channels(&mut self, path: &Path) -> Channels {
        match self.probe(path) {
            Probe::Reading => Channels::Reading,
            Probe::Read { info, .. } => info
                .streams
                .iter()
                .filter(|stream| stream.kind == StreamKind::Audio)
                .find_map(|stream| stream.channels)
                .map_or(Channels::Read(0), |channels| {
                    Channels::Read(u16::try_from(channels).unwrap_or(u16::MAX))
                }),
            Probe::Failed(error) => Channels::Failed(error.clone()),
        }
    }

    /// Follows whichever of the project and timeline selections changed last.
    fn subject(&mut self, item: Option<SourceId>, clip: Option<ClipId>) -> Follow {
        if item != self.seen_item {
            self.seen_item = item;
            self.follow = Follow::Project;
        }
        if clip != self.seen_clip {
            self.seen_clip = clip;
            self.follow = Follow::Timeline;
        }
        match (self.follow, item, clip) {
            (Follow::Timeline, _, Some(_)) | (Follow::Project, None, Some(_)) => Follow::Timeline,
            _ => Follow::Project,
        }
    }
}

/// The Metadata panel: the selected clip and its media's properties, and the log notes
/// kept about the media.
pub(crate) fn ui(editor: &mut Editor, ui: &mut egui::Ui) {
    editor.metadata.poll();
    let sequence = editor.timeline.sequence();
    let clip = editor.view.selection.iter().copied().find(|&id| {
        sequence
            .clip(id)
            .is_some_and(|(_, clip)| sequence.media_item(clip.source).is_some())
    });
    let item = editor.pool.selected.last().copied();
    let follow = editor.metadata.subject(item, clip);
    let source = match (follow, clip) {
        (Follow::Timeline, Some(id)) => sequence.clip(id).map(|(_, clip)| clip.source),
        _ => item,
    };
    let Some(source) = source else {
        ui.label(RichText::new("Select a clip or a project item to see its metadata.").weak());
        return;
    };
    let mut committed = None;
    egui::ScrollArea::vertical()
        .auto_shrink([false, true])
        .show(ui, |ui| {
            if follow == Follow::Timeline {
                if let Some(id) = clip {
                    clip_section(editor, ui, id);
                }
            }
            let Some(media) = editor.timeline.project().media_item(source).cloned() else {
                return;
            };
            file_section(editor, ui, &media);
            committed = log_section(editor, ui, &media);
        });
    if let Some((source, log)) = committed {
        editor.apply_to_pool(SetMediaLog { source, log });
    }
}

fn clip_section(editor: &Editor, ui: &mut egui::Ui, id: ClipId) {
    let sequence = editor.timeline.sequence();
    let Some((track, clip)) = sequence.clip(id) else {
        return;
    };
    let (timebase, rate, display) = (
        sequence.timebase,
        sequence.format.frame_rate,
        sequence.time_display,
    );
    let timecode = |ticks: Ticks| Timecode::shown(ticks, timebase, rate, display).to_string();
    let track_name = match sequence
        .track(track)
        .and_then(|named| named.name.as_deref())
    {
        Some(name) => format!("{track} · {name}"),
        None => track.to_string(),
    };
    egui::CollapsingHeader::new("Clip")
        .default_open(true)
        .show(ui, |ui| {
            properties(ui, "metadata_clip", |row| {
                row("Track", track_name);
                row("Sequence In", timecode(clip.start + sequence.start_time));
                row("Sequence Out", timecode(clip.end() + sequence.start_time));
                row("Duration", timecode(clip.duration));
                let media_start = sequence
                    .media_item(clip.source)
                    .map_or(Ticks::ZERO, |item| item.start_timecode(timebase));
                row("Media In", timecode(media_start + clip.source_in));
                row("Media Out", timecode(media_start + clip.source_out()));
                let mut speed = format!("{:.2}%", clip.speed.percent());
                if clip.reverse {
                    speed.push_str(", reversed");
                }
                row("Speed", speed);
                row("Enabled", yes_no(!clip.disabled));
            });
        });
}

fn file_section(editor: &mut Editor, ui: &mut egui::Ui, media: &MediaItem) {
    let project = editor.timeline.project();
    let (timebase, rate, display) = (
        project.timebase,
        project.format.frame_rate,
        project.time_display,
    );
    ui.add(egui::Label::new(RichText::new(&media.name).heading()).truncate());
    let probe = media.is_file().then(|| editor.metadata.probe(&media.path));
    egui::CollapsingHeader::new("File")
        .default_open(true)
        .show(ui, |ui| {
            properties(ui, "metadata_file", |row| {
                row("Type", kind(media).to_owned());
                if media.is_file() {
                    row("File Path", media.path.display().to_string());
                }
                if let Some(video) = media.video {
                    row("Frame Size", format!("{} × {}", video.width, video.height));
                    row("Frame Rate", fps(video.frame_rate));
                    row(
                        "Pixel Aspect",
                        format!("{:.4}", media.pixel_aspect().ratio()),
                    );
                    row("Color Space", media.color_space().name().to_owned());
                    row("Field Order", media.field_order().name().to_owned());
                }
                if let Some(assumed) = media.interpreted_rate {
                    row("Interpreted As", fps(assumed));
                }
                if !media.alpha.is_straight() {
                    row("Alpha", media.alpha.name().to_owned());
                }
                row(
                    "Duration",
                    duration(media.duration, timebase, rate, display),
                );
                if let (Some(_), Some(format)) = (media.timecode, media.played_format()) {
                    let start = media.start_timecode(timebase);
                    let shown = Timecode::shown(start, timebase, format.frame_rate, display);
                    row("Media Start", shown.to_string());
                }
                if let Some(range) = &media.subclip {
                    let timecode =
                        |ticks| Timecode::shown(ticks, timebase, rate, display).to_string();
                    row(
                        "Subclip",
                        format!("{} – {}", timecode(range.start), timecode(range.end)),
                    );
                }
                row("Audio", yes_no(media.has_audio));
                if let Some(proxy) = &media.proxy {
                    row("Proxy", proxy.display().to_string());
                }
                match probe {
                    None => {}
                    Some(Probe::Reading) => row("Format", "Reading…".to_owned()),
                    Some(Probe::Failed(message)) => row("Format", message.clone()),
                    Some(Probe::Read { info, bytes }) => {
                        row("Format", info.container.clone());
                        row("File Size", size(*bytes));
                        if let Some(bits) = info.bit_rate {
                            row("Data Rate", bit_rate(bits));
                        }
                        for stream in &info.streams {
                            if let Some(name) = stream_name(stream.kind) {
                                row(name, stream_text(stream));
                            }
                        }
                    }
                }
            });
        });
}

/// Names beside their values, or above them when the panel is too narrow for both.
fn properties(ui: &mut egui::Ui, id: &str, rows: impl FnOnce(&mut dyn FnMut(&str, String))) {
    if ui.available_width() < SIDE_BY_SIDE_WIDTH {
        rows(&mut |name, value| {
            ui.label(RichText::new(name).weak().small());
            ui.add(egui::Label::new(value).wrap());
            ui.add_space(2.0);
        });
    } else {
        Grid::new(id).num_columns(2).show(ui, |ui| {
            rows(&mut |name, value| {
                ui.label(RichText::new(name).weak());
                ui.add(egui::Label::new(value).wrap());
                ui.end_row();
            });
        });
    }
}

/// Returns the log to keep, and whose it is, once a field is left changed or the item
/// shown changes under a field being typed in.
fn log_section(
    editor: &mut Editor,
    ui: &mut egui::Ui,
    media: &MediaItem,
) -> Option<(SourceId, MediaLog)> {
    let project = editor.timeline.project();
    let editing = &mut editor.metadata.editing;
    let mut switched = None;
    if editing
        .as_ref()
        .is_none_or(|(source, _)| *source != media.id)
    {
        switched = editing.take().filter(|(source, log)| {
            project
                .media_item(*source)
                .is_some_and(|item| item.log != *log)
        });
        *editing = Some((media.id, media.log.clone()));
    }
    let (_, log) = editing.as_mut()?;
    let mut left = false;
    let mut typing = false;
    egui::CollapsingHeader::new("Log")
        .default_open(true)
        .show(ui, |ui| {
            Grid::new("metadata_log").num_columns(2).show(ui, |ui| {
                let fields = [
                    ("Description", &mut log.description, 3),
                    ("Scene", &mut log.scene, 1),
                    ("Shot", &mut log.shot, 1),
                    ("Log Note", &mut log.log_note, 2),
                ];
                for (name, text, rows) in fields {
                    ui.label(RichText::new(name).weak());
                    let field = egui::TextEdit::multiline(text)
                        .desired_rows(rows)
                        .desired_width(f32::INFINITY);
                    let response = ui.add(field);
                    left |= response.lost_focus();
                    typing |= response.has_focus();
                    ui.end_row();
                }
                ui.label(RichText::new("Good").weak());
                left |= ui.checkbox(&mut log.good, "").changed();
                ui.end_row();
            });
        });
    if switched.is_some() {
        return switched;
    }
    if left && *log != media.log {
        return Some((media.id, log.clone()));
    }
    // Undo and redo change the log underneath; show theirs unless a field is in use.
    if !typing {
        log.clone_from(&media.log);
    }
    None
}

fn kind(media: &MediaItem) -> &'static str {
    match (&media.generator, &media.nest, &media.subclip) {
        (Some(generator), _, _) => generator.kind_name(),
        (None, Some(_), _) => "Sequence",
        (None, None, Some(_)) => "Subclip",
        (None, None, None) => match (media.video.is_some(), media.has_audio) {
            (true, true) => "Movie",
            (true, false) => "Video",
            (false, _) => "Audio",
        },
    }
}

fn fps(rate: FrameRate) -> String {
    let fps = format!("{:.3}", rate.approx_fps());
    format!("{} fps", fps.trim_end_matches('0').trim_end_matches('.'))
}

/// Generated media is endless until placed.
fn duration(ticks: Ticks, timebase: Timebase, rate: FrameRate, display: TimeDisplay) -> String {
    if ticks.0 == i64::MAX {
        "Unlimited".to_owned()
    } else {
        Timecode::shown(ticks, timebase, rate, display).to_string()
    }
}

fn yes_no(yes: bool) -> String {
    if yes { "Yes" } else { "No" }.to_owned()
}

pub(crate) fn size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = "bytes";
    for next in UNITS {
        if value < 1024.0 {
            break;
        }
        value /= 1024.0;
        unit = next;
    }
    if unit == "bytes" {
        format!("{bytes} bytes")
    } else {
        format!("{value:.1} {unit}")
    }
}

fn bit_rate(bits: u64) -> String {
    match bits {
        0..1_000_000 => format!("{:.0} kb/s", bits as f64 / 1e3),
        _ => format!("{:.1} Mb/s", bits as f64 / 1e6),
    }
}

fn stream_name(kind: StreamKind) -> Option<&'static str> {
    match kind {
        StreamKind::Video => Some("Video Stream"),
        StreamKind::Audio => Some("Audio Stream"),
        StreamKind::Subtitle => Some("Subtitle Stream"),
        StreamKind::Other => None,
    }
}

fn stream_text(stream: &StreamInfo) -> String {
    let mut parts = vec![stream.codec.clone()];
    if let Some(rate) = stream.sample_rate {
        parts.push(format!("{rate} Hz"));
    }
    if let Some(channels) = stream.channels {
        parts.push(match channels {
            1 => "mono".to_owned(),
            2 => "stereo".to_owned(),
            n => format!("{n} channels"),
        });
    }
    if let Some(bits) = stream.bit_rate {
        parts.push(bit_rate(bits));
    }
    parts.join(", ")
}
