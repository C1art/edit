use std::time::{Duration, Instant};

use audio::{AudioError, AudioOutput, OutputDevice};
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
    pub(crate) chosen: Option<OutputDevice>,
    opening: Option<Opening>,
    /// The choice the output in use was opened for.
    opened_for: Option<Option<OutputDevice>>,
    retry_at: Option<Instant>,
    /// Whether the user was told no device could be opened, so it's said once, not on
    /// every retry.
    told_failure: bool,
    listing: Option<Receiver<Vec<OutputDevice>>>,
    listed_at: Option<Instant>,
    devices: Vec<OutputDevice>,
}

struct Opening {
    choice: Option<OutputDevice>,
    output: Receiver<Result<AudioOutput, AudioError>>,
}

impl AudioSwitch {
    /// The worker wakes the editor when it's done, so nothing polls meanwhile.
    fn open(&mut self, ctx: &egui::Context) -> std::io::Result<()> {
        let (sender, output) = channel::bounded(1);
        let choice = self.chosen.clone();
        let wanted = choice.clone();
        let ctx = ctx.clone();
        std::thread::Builder::new()
            .name("audio open".to_owned())
            .spawn(move || {
                let opened = AudioOutput::open(wanted.as_ref()).or_else(|error| match error {
                    AudioError::MissingDevice(_) => AudioOutput::open(None),
                    error => Err(error),
                });
                let _ = sender.send(opened);
                ctx.request_repaint();
            })?;
        self.opening = Some(Opening { choice, output });
        Ok(())
    }

    /// The output devices last found, asking the system again on a worker when the list
    /// has gone stale.
    pub(crate) fn devices(&mut self) -> &[OutputDevice] {
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
        if let Err(error) = switch.open(ctx) {
            switch.retry_at = Some(Instant::now() + RETRY);
            self.notices.push(format!("No sound: {error}"));
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
                let device = &output.device().name;
                if let Some(wanted) = choice.as_ref().filter(|wanted| !output.device().is(wanted)) {
                    self.notices.push(format!(
                        "The audio device “{}” is not connected; playing through “{device}”",
                        wanted.name
                    ));
                } else if switch.told_failure || (self.audio.is_some() && in_use.is_none()) {
                    self.notices
                        .push(format!("Audio now plays through “{device}”"));
                }
                switch.told_failure = false;
                switch.retry_at = None;
                // Remembered as opened, so a choice kept by name alone gains its id and a
                // renumbered name is brought up to date.
                if choice
                    .as_ref()
                    .is_some_and(|wanted| output.device().is(wanted))
                {
                    switch.chosen = Some(output.device().clone());
                }
                switch.opened_for = Some(switch.chosen.clone());
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
                    self.notices.push(format!("No sound: {error}"));
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
        let mut players: Vec<_> = [
            self.program.as_mut().map(|program| &mut program.player),
            self.source.as_mut().map(|source| &mut source.viewer.player),
        ]
        .into_iter()
        .flatten()
        .collect();
        // Each new writer takes the device from the one before, so the player that is
        // playing goes last and keeps it.
        players.sort_by_key(|player| player.is_playing());
        for player in players {
            if let Err(error) = player.set_output(output.as_ref().map(AudioOutput::writer)) {
                self.notices.push(format!("No sound: {error}"));
            }
        }
        self.audio = output;
    }
}

pub(crate) fn menu(editor: &mut Editor, ui: &mut egui::Ui) {
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
    let switch = &mut editor.audio_switch;
    let in_use = editor.audio.as_ref().map(|output| &output.device().name);
    let mut picked = None;
    let default = match (&switch.chosen, in_use) {
        (None, Some(device)) => format!("System Default ({device})"),
        _ => "System Default".to_owned(),
    };
    if ui.radio(switch.chosen.is_none(), default).clicked() {
        picked = Some(None);
    }
    let chosen = switch.chosen.clone();
    let is_chosen = |device: &OutputDevice| chosen.as_ref().is_some_and(|chosen| chosen.is(device));
    let devices = switch.devices();
    if devices.is_empty() {
        ui.weak("Looking for devices…");
    }
    for device in devices {
        if ui.radio(is_chosen(device), &device.name).clicked() {
            picked = Some(Some(device.clone()));
        }
    }
    if let Some(missing) = chosen
        .as_ref()
        .filter(|chosen| !switch.devices.iter().any(|device| device.is(chosen)))
    {
        let _ = ui.radio(true, format!("{} (not connected)", missing.name));
    }
    // Compared as devices, so picking the one in use again, or a device remembered by
    // name alone, doesn't reopen it.
    let changed = |picked: &Option<OutputDevice>| match (picked, &chosen) {
        (Some(picked), Some(chosen)) => !picked.is(chosen),
        (picked, chosen) => picked.is_some() != chosen.is_some(),
    };
    if let Some(picked) = picked.filter(changed) {
        switch.chosen = picked;
        ui.close();
    }
}
