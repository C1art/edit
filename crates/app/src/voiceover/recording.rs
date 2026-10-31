//! The worker that holds the microphone open and writes what it hears to a WAV file.

use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::time::Duration;

use audio::{AudioInput, InputDevice, InputLevel, WavWriter};
use crossbeam::channel::{self, Receiver, RecvTimeoutError, Sender};
use timeline::{FrameRate, ImportMedia, Timebase};

/// How often the worker moves sound from the input queue to the file.
const DRAIN_EVERY: Duration = Duration::from_millis(50);

pub(crate) enum Command {
    /// Starts keeping what the microphone hears.
    Start,
    /// Ends the take and keeps it.
    Stop,
}

pub(crate) enum Event {
    Opened(InputLevel),
    /// The take, written and probed, ready to join the project, with why it may be
    /// shorter than expected.
    Finished {
        media: ImportMedia,
        warning: Option<&'static str>,
    },
    Failed(String),
}

pub(crate) struct Recording {
    pub(crate) commands: Sender<Command>,
    pub(crate) events: Receiver<Event>,
}

/// Where a take goes and how it's read back.
pub(crate) struct Destination {
    pub(crate) folder: PathBuf,
    pub(crate) timebase: Timebase,
    pub(crate) frame_rate: FrameRate,
}

impl Recording {
    /// Opens `device`, or the system's default input, on a worker. Dropping the
    /// recording before stopping it throws the take away.
    pub(crate) fn open(
        device: Option<InputDevice>,
        destination: Destination,
        ctx: &egui::Context,
    ) -> std::io::Result<Self> {
        let (commands, command_rx) = channel::unbounded();
        let (event_tx, events) = channel::unbounded();
        let ctx = ctx.clone();
        std::thread::Builder::new()
            .name("voiceover".to_owned())
            .spawn(move || {
                let event = record(device.as_ref(), &destination, &command_rx, &event_tx);
                if let Some(event) = event {
                    let _ = event_tx.send(event);
                }
                ctx.request_repaint();
            })?;
        Ok(Self { commands, events })
    }
}

fn record(
    device: Option<&InputDevice>,
    destination: &Destination,
    commands: &Receiver<Command>,
    events: &Sender<Event>,
) -> Option<Event> {
    let input = match AudioInput::open(device) {
        Ok(input) => input,
        Err(error) => return Some(Event::Failed(format!("Cannot record: {error}"))),
    };
    let _ = events.send(Event::Opened(input.level()));
    let mut take: Option<(PathBuf, WavWriter<BufWriter<File>>)> = None;
    let mut samples = Vec::new();
    loop {
        let stop = match commands.recv_timeout(DRAIN_EVERY) {
            Ok(Command::Start) if take.is_none() => {
                match start(&destination.folder, &input) {
                    Ok(started) => take = Some(started),
                    Err(error) => return Some(Event::Failed(format!("Cannot record: {error}"))),
                }
                false
            }
            Ok(Command::Start) | Err(RecvTimeoutError::Timeout) => false,
            Ok(Command::Stop) => true,
            Err(RecvTimeoutError::Disconnected) => {
                if let Some((path, writer)) = take {
                    drop(writer);
                    let _ = std::fs::remove_file(path);
                }
                return None;
            }
        };
        let Some((path, writer)) = &mut take else {
            if stop {
                return None;
            }
            continue;
        };
        input.drain(&mut samples);
        // A full disk shouldn't cost what was already recorded, so the take ends there.
        let written = writer.write(&samples).is_ok();
        samples.clear();
        if stop || input.is_lost() || !written {
            let path = path.clone();
            let (_, writer) = take?;
            return Some(match finish(path, writer, destination, &input) {
                Event::Finished { media, .. } if !written => Event::Finished {
                    media,
                    warning: Some("Writing the voiceover failed; it ends there"),
                },
                event => event,
            });
        }
    }
}

/// Creates the file for a take and starts keeping sound for it.
fn start(
    folder: &Path,
    input: &AudioInput,
) -> std::io::Result<(PathBuf, WavWriter<BufWriter<File>>)> {
    std::fs::create_dir_all(folder)?;
    let path = (1..)
        .map(|number| folder.join(format!("Voiceover {number:02}.wav")))
        .find(|path| !path.exists())
        .unwrap_or_else(|| folder.join("Voiceover.wav"));
    let file = BufWriter::new(File::create(&path)?);
    let writer = WavWriter::new(file, input.sample_rate())?;
    input.keep();
    Ok((path, writer))
}

fn finish(
    path: PathBuf,
    writer: WavWriter<BufWriter<File>>,
    destination: &Destination,
    input: &AudioInput,
) -> Event {
    // The file is left for the user to salvage what it holds.
    if let Err(error) = writer.finish() {
        return Event::Failed(format!(
            "Cannot finish the voiceover {}: {error}",
            path.display()
        ));
    }
    // What came in before the microphone went away or the computer fell behind is
    // still kept.
    let warning = if input.is_lost() {
        Some("The microphone stopped; the voiceover ends there")
    } else if input.overflowed() {
        Some("Some of the voiceover was lost: the computer fell behind")
    } else {
        None
    };
    match crate::import::probe(&path, destination.timebase, destination.frame_rate) {
        Ok(media) => Event::Finished { media, warning },
        Err(error) => Event::Failed(format!(
            "Cannot read the voiceover {} back: {error}",
            path.display()
        )),
    }
}
