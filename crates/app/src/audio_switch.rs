use std::time::{Duration, Instant};

use audio::{AudioError, AudioOutput};
use crossbeam::channel::{self, Receiver, TryRecvError};

use crate::editor::Editor;

/// How long to wait before trying a device again after none could be opened, so one
/// plugged in later is picked up.
const RETRY: Duration = Duration::from_secs(5);
/// How long a device list stays fresh while the Preferences menu shows it.
const RELIST: Duration = Duration::from_secs(2);

/// Opens the audio device on a worker, since that can take a while, and moves playback to
/// it: at start, when another device is chosen, and when the one in use goes away.
#[derive(Default)]
pub(crate) struct AudioSwitch {
    /// The device chosen in Preferences; `None` follows the system default.
    pub(crate) chosen: Option<String>,
    opening: Option<Opening>,
    /// The choice the output in use was opened for.
    opened_for: Option<Option<String>>,
    retry_at: Option<Instant>,
    /// Whether the user was told no device could be opened, so it's said once, not on
    /// every retry.
    told_failure: bool,
    listing: Option<Receiver<Vec<String>>>,
    listed_at: Option<Instant>,
    devices: Vec<String>,
}

struct Opening {
    choice: Option<String>,
    output: Receiver<Result<AudioOutput, AudioError>>,
}

impl AudioSwitch {
    fn open(&mut self) -> std::io::Result<()> {
        let (sender, output) = channel::bounded(1);
        let choice = self.chosen.clone();
        let wanted = choice.clone();
        std::thread::Builder::new()
            .name("audio open".to_owned())
            .spawn(move || {
                let opened = AudioOutput::open(wanted.as_deref()).or_else(|error| match error {
                    AudioError::MissingDevice(_) => AudioOutput::open(None),
                    error => Err(error),
                });
                let _ = sender.send(opened);
            })?;
        self.opening = Some(Opening { choice, output });
        Ok(())
    }

    /// The output devices last found, asking the system again on a worker when the list
    /// has gone stale.
    pub(crate) fn devices(&mut self) -> &[String] {
        if let Some(listing) = &self.listing {
            match listing.try_recv() {
                Ok(devices) => self.devices = devices,
                Err(TryRecvError::Empty) => return &self.devices,
                Err(TryRecvError::Disconnected) => {}
            }
            self.listing = None;
            self.listed_at = Some(Instant::now());
        }
        if self.listed_at.is_none_or(|at| at.elapsed() >= RELIST) {
            let (sender, listing) = channel::bounded(1);
            let spawned = std::thread::Builder::new()
                .name("audio devices".to_owned())
                .spawn(move || {
                    let _ = sender.send(audio::output_devices().unwrap_or_default());
                });
            match spawned {
                Ok(_) => self.listing = Some(listing),
                Err(_) => self.listed_at = Some(Instant::now()),
            }
        }
        &self.devices
    }
}

impl Editor {
    /// Opens or reopens the audio device when needed and moves playback onto it once open.
    pub(crate) fn tend_audio_output(&mut self, ctx: &egui::Context) {
        self.receive_output();
        let switch = &mut self.audio_switch;
        if switch.opening.is_some() {
            ctx.request_repaint_after(Duration::from_millis(50));
            return;
        }
        let reopen = match &self.audio {
            Some(output) => output.is_lost() || switch.opened_for.as_ref() != Some(&switch.chosen),
            None => switch.retry_at.is_none_or(|at| Instant::now() >= at),
        };
        if !reopen {
            if self.audio.is_none() {
                ctx.request_repaint_after(RETRY);
            }
            return;
        }
        match switch.open() {
            Ok(()) => ctx.request_repaint_after(Duration::from_millis(50)),
            Err(error) => {
                switch.retry_at = Some(Instant::now() + RETRY);
                self.notices.push(format!("Audio disabled: {error}"));
            }
        }
    }

    fn receive_output(&mut self) {
        let switch = &mut self.audio_switch;
        let Some(opening) = &switch.opening else {
            return;
        };
        let opened = match opening.output.try_recv() {
            Ok(opened) => opened,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err(AudioError::NoOutputDevice),
        };
        let choice = opening.choice.clone();
        switch.opening = None;
        if choice != switch.chosen {
            // Chosen again meanwhile: the next tend opens what's wanted now.
            return;
        }
        let in_use = self.audio.as_ref().filter(|output| !output.is_lost());
        match opened {
            Ok(output) => {
                let device = output.device_name();
                if let Some(name) = choice.as_deref().filter(|&name| name != device) {
                    self.notices.push(format!(
                        "The audio device “{name}” is not connected; playing through “{device}”"
                    ));
                } else if switch.told_failure || (self.audio.is_some() && in_use.is_none()) {
                    self.notices
                        .push(format!("Audio now plays through “{device}”"));
                }
                switch.told_failure = false;
                switch.retry_at = None;
                switch.opened_for = Some(choice);
                self.use_output(Some(output));
            }
            // Another device was chosen and couldn't be opened: keep playing through this one.
            Err(error) if in_use.is_some() => {
                self.notices
                    .push(format!("Audio device unchanged: {error}"));
                switch.opened_for = Some(choice);
            }
            Err(error) => {
                if !switch.told_failure {
                    self.notices.push(format!("Audio disabled: {error}"));
                    switch.told_failure = true;
                }
                switch.retry_at = Some(Instant::now() + RETRY);
                if self.audio.is_some() {
                    // Plays silently on the wall clock until a device can be opened again.
                    self.use_output(None);
                }
            }
        }
    }

    fn use_output(&mut self, output: Option<AudioOutput>) {
        let players = [
            self.program.as_mut().map(|program| &mut program.player),
            self.source.as_mut().map(|source| &mut source.viewer.player),
        ];
        for player in players.into_iter().flatten() {
            if let Err(error) = player.set_output(output.as_ref().map(AudioOutput::writer)) {
                self.notices.push(format!("Audio disabled: {error}"));
            }
        }
        self.audio = output;
    }
}

pub(crate) fn menu(editor: &mut Editor, ui: &mut egui::Ui) {
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
    let switch = &mut editor.audio_switch;
    let in_use = editor.audio.as_ref().map(AudioOutput::device_name);
    let mut chosen = switch.chosen.clone();
    let default = match (&switch.chosen, in_use) {
        (None, Some(device)) => format!("System Default ({device})"),
        _ => "System Default".to_owned(),
    };
    ui.radio_value(&mut chosen, None, default);
    let devices = switch.devices();
    if devices.is_empty() {
        ui.weak("Looking for devices…");
    }
    for device in devices {
        ui.radio_value(&mut chosen, Some(device.clone()), device);
    }
    if let Some(missing) = switch
        .chosen
        .as_ref()
        .filter(|&name| !switch.devices.contains(name))
    {
        ui.radio_value(
            &mut chosen,
            Some(missing.clone()),
            format!("{missing} (not connected)"),
        );
    }
    if chosen != switch.chosen {
        switch.chosen = chosen;
        ui.close();
    }
}
