use std::path::{Path, PathBuf};

use audio::AudioOutput;
use egui::{CentralPanel, Key, KeyboardShortcut, MenuBar, Modifiers, Panel, ViewportCommand};
use playback::Player;
use timeline::{Sequence, Timebase, Timecode, Timeline, VideoFormat};

use crate::monitor::Monitor;
use crate::transport;

const OPEN_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);

pub struct Editor {
    timeline: Timeline,
    audio: Option<AudioOutput>,
    monitor: Option<Monitor>,
    player: Option<Player>,
    source_path: Option<PathBuf>,
    /// The path being typed into the open dialog, while it is showing.
    open_prompt: Option<String>,
    /// Problems worth telling the user about, newest last.
    notices: Vec<String>,
}

impl Editor {
    pub fn new(creation_context: &eframe::CreationContext<'_>, initial: Option<PathBuf>) -> Self {
        let mut notices = Vec::new();
        let audio = AudioOutput::open_default()
            .map_err(|error| notices.push(format!("Audio disabled: {error}")))
            .ok();
        let monitor = creation_context.wgpu_render_state.clone().map(Monitor::new);
        if monitor.is_none() {
            notices.push("Video disabled: the window has no wgpu renderer".to_owned());
        }
        let mut editor = Self {
            timeline: Timeline::new(Sequence::with_default_tracks(
                Timebase::HZ_90K,
                VideoFormat::default(),
            )),
            audio,
            monitor,
            player: None,
            source_path: None,
            open_prompt: None,
            notices,
        };
        if let Some(path) = initial {
            editor.open(path);
        }
        editor
    }

    fn timebase(&self) -> Timebase {
        self.timeline.sequence().timebase
    }

    fn open(&mut self, path: PathBuf) {
        // Drop the old player first so its worker stops feeding the audio queue.
        self.player = None;
        let writer = self.audio.as_ref().map(AudioOutput::writer);
        match Player::open(path.clone(), self.timebase(), writer) {
            Ok(player) => {
                self.player = Some(player);
                self.source_path = Some(path);
            }
            Err(error) => self
                .notices
                .push(format!("Cannot open {}: {error}", path.display())),
        }
    }

    fn update_playback(&mut self, ctx: &egui::Context) {
        let Some(player) = &mut self.player else {
            return;
        };
        transport::handle_shortcuts(ctx, player);
        if player.update() {
            if let (Some(monitor), Some(frame)) = (&mut self.monitor, player.current_frame()) {
                if let Err(error) = monitor.present(frame) {
                    self.notices.push(format!("Cannot display frame: {error}"));
                }
            }
        }
        if player.is_busy() {
            ctx.request_repaint();
        }
    }

    fn take_dropped_file(ctx: &egui::Context) -> Option<PathBuf> {
        ctx.input(|input| {
            input
                .raw
                .dropped_files
                .first()
                .map(|file| file.path().to_owned())
                .filter(|path| !path.as_os_str().is_empty())
        })
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                let open = egui::Button::new("Open…")
                    .shortcut_text(ui.ctx().format_shortcut(&OPEN_SHORTCUT));
                if ui.add(open).clicked() {
                    self.open_prompt = Some(String::new());
                }
                if ui.button("Quit").clicked() {
                    ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                }
            });
        });
    }

    fn open_dialog(&mut self, ctx: &egui::Context) {
        let Some(prompt) = &mut self.open_prompt else {
            return;
        };
        let mut submitted = None;
        let mut cancelled = false;
        egui::Window::new("Open media")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Path to a video or audio file (or drop a file on the window):");
                let field = ui.add(egui::TextEdit::singleline(prompt).desired_width(480.0));
                field.request_focus();
                let entered = field.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter));
                ui.horizontal(|ui| {
                    if ui.button("Open").clicked() || entered {
                        submitted = Some(PathBuf::from(prompt.trim().trim_matches('"')));
                    }
                    if ui.button("Cancel").clicked()
                        || ui.input(|input| input.key_pressed(Key::Escape))
                    {
                        cancelled = true;
                    }
                });
            });
        if let Some(path) = submitted.filter(|path| !path.as_os_str().is_empty()) {
            self.open_prompt = None;
            self.open(path);
        } else if cancelled {
            self.open_prompt = None;
        }
    }

    fn media_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Media");
        ui.separator();
        match (&self.source_path, &self.player) {
            (Some(path), Some(player)) => source_summary(ui, path, player, self.timebase()),
            _ => {
                ui.label("Drop a file here or use File → Open.");
            }
        }
        if !self.notices.is_empty() {
            ui.separator();
            for notice in &self.notices {
                ui.colored_label(ui.visuals().warn_fg_color, notice);
            }
            if ui.small_button("Clear messages").clicked() {
                self.notices.clear();
            }
        }
    }
}

fn source_summary(ui: &mut egui::Ui, path: &Path, player: &Player, timebase: Timebase) {
    let name = path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    ui.strong(name);
    if let Some(error) = player.error() {
        ui.colored_label(ui.visuals().error_fg_color, error.to_string());
        return;
    }
    let Some(info) = player.info() else {
        ui.spinner();
        return;
    };
    let rate = transport::frame_rate(player);
    if let Some(duration) = info.duration {
        ui.label(format!(
            "Duration {}",
            Timecode::new(duration, timebase, rate)
        ));
    }
    if let Some(video) = &info.video {
        let size = player
            .current_frame()
            .map(|frame| format!("{}×{}", frame.width, frame.height));
        let fps = video.frame_rate.map(|rate| {
            let time = rate.frame_timebase();
            format!("{:.3} fps", f64::from(time.den()) / f64::from(time.num()))
        });
        let details: Vec<_> = [size, fps].into_iter().flatten().collect();
        ui.label(format!("Video {}", details.join(", ")));
    }
    ui.label(if info.has_audio {
        "Audio stereo"
    } else {
        "No audio"
    });
}

impl eframe::App for Editor {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(path) = Self::take_dropped_file(&ctx) {
            self.open(path);
        }
        if ctx.input_mut(|input| input.consume_shortcut(&OPEN_SHORTCUT)) {
            self.open_prompt = Some(String::new());
        }
        self.update_playback(&ctx);

        Panel::top("menu_bar").show(ui, |ui| self.menu_bar(ui));

        Panel::left("media_pool")
            .resizable(true)
            .default_size(260.0)
            .show(ui, |ui| self.media_panel(ui));

        Panel::bottom("timeline")
            .resizable(true)
            .default_size(280.0)
            .show(ui, |ui| {
                ui.heading("Timeline");
                ui.label(format!(
                    "{} tracks",
                    self.timeline.sequence().tracks().count()
                ));
            });

        let timebase = self.timebase();
        CentralPanel::default().show(ui, |ui| {
            if let Some(player) = &mut self.player {
                Panel::bottom("transport").show(ui, |ui| {
                    transport::ui(ui, player, timebase);
                });
            }
            match &self.monitor {
                Some(monitor) => monitor.ui(ui),
                None => {
                    ui.centered_and_justified(|ui| ui.label("Program monitor"));
                }
            }
        });

        self.open_dialog(&ctx);
    }
}
