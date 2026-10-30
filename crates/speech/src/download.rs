use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam::channel::{Receiver, Sender};

use crate::{Model, SpeechError};

/// How often the partial file is measured for progress.
const POLL: Duration = Duration::from_millis(250);

#[derive(Debug)]
pub enum DownloadEvent {
    /// The fraction downloaded so far, roughly.
    Progress(f32),
    Done,
    Failed(SpeechError),
}

/// Downloads a model into the models folder with the system's `curl`, which Windows
/// ships, to a partial file renamed into place once whole. Dropping the job cancels it.
pub struct DownloadJob {
    events: Receiver<DownloadEvent>,
    cancel: Arc<AtomicBool>,
}

impl DownloadJob {
    pub fn start(model: Model, models: &Path) -> Result<Self, SpeechError> {
        let (sender, events) = crossbeam::channel::unbounded();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let target = model.path(models);
        thread::Builder::new()
            .name("model download".into())
            .spawn(move || {
                let event = match download(model, &target, &sender, &worker_cancel) {
                    Ok(true) => DownloadEvent::Done,
                    Ok(false) => return,
                    Err(error) => DownloadEvent::Failed(error),
                };
                let _ = sender.send(event);
            })
            .map_err(SpeechError::WorkerSpawn)?;
        Ok(Self { events, cancel })
    }

    /// Everything that happened since the last poll; never blocks.
    pub fn poll(&self) -> Vec<DownloadEvent> {
        self.events.try_iter().collect()
    }
}

impl Drop for DownloadJob {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// Whether the model arrived; `false` when cancelled.
fn download(
    model: Model,
    target: &Path,
    events: &Sender<DownloadEvent>,
    cancel: &AtomicBool,
) -> Result<bool, SpeechError> {
    let failed = |error: std::io::Error| SpeechError::Download(error.to_string());
    if let Some(folder) = target.parent() {
        std::fs::create_dir_all(folder).map_err(failed)?;
    }
    let mut part = target.as_os_str().to_owned();
    part.push(".part");
    let part = PathBuf::from(part);
    let mut curl = curl(&model.url(), &part)
        .map_err(|error| SpeechError::Download(format!("could not run curl: {error}")))?;
    let status = loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = curl.kill();
            let _ = curl.wait();
            let _ = std::fs::remove_file(&part);
            return Ok(false);
        }
        if let Some(status) = curl.try_wait().map_err(failed)? {
            break status;
        }
        let downloaded = std::fs::metadata(&part).map_or(0, |file| file.len());
        let fraction = downloaded as f32 / model.approximate_bytes() as f32;
        let _ = events.send(DownloadEvent::Progress(fraction.min(0.99)));
        thread::sleep(POLL);
    };
    if !status.success() {
        let mut message = String::new();
        if let Some(mut stderr) = curl.stderr.take() {
            let _ = stderr.read_to_string(&mut message);
        }
        let _ = std::fs::remove_file(&part);
        let message = message.trim();
        return Err(SpeechError::Download(if message.is_empty() {
            format!("curl stopped with {status}")
        } else {
            message.to_owned()
        }));
    }
    std::fs::rename(&part, target).map_err(failed)?;
    Ok(true)
}

fn curl(url: &str, into: &Path) -> std::io::Result<Child> {
    let mut command = Command::new("curl");
    command
        .args([
            "--location",
            "--fail",
            "--silent",
            "--show-error",
            "--retry",
            "3",
        ])
        .arg("--output")
        .arg(into)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Without it a console window flashes up over the editor.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command.spawn()
}
