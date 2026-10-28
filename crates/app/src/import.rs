use std::path::PathBuf;
use std::thread;

use crossbeam::channel::{self, Receiver, Sender};
use media::{MediaError, MediaReader};
use timeline::{FrameRate, ImportMedia, Timebase, VideoFormat};

/// Rate assumed for video whose container declares none.
const FALLBACK_RATE: FrameRate = FrameRate::FPS_25;

pub type ImportResult = Result<ImportMedia, (PathBuf, MediaError)>;

/// Probes files on a worker thread so the UI never touches the disk.
pub struct Importer {
    requests: Sender<PathBuf>,
    results: Receiver<ImportResult>,
    pending: usize,
}

impl Importer {
    pub fn new(timebase: Timebase) -> std::io::Result<Self> {
        let (request_tx, request_rx) = channel::unbounded::<PathBuf>();
        let (result_tx, result_rx) = channel::unbounded();
        thread::Builder::new()
            .name("import".to_owned())
            .spawn(move || {
                for path in request_rx {
                    let result = probe(&path, timebase).map_err(|error| (path, error));
                    if result_tx.send(result).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            requests: request_tx,
            results: result_rx,
            pending: 0,
        })
    }

    pub fn request(&mut self, path: PathBuf) {
        if self.requests.send(path).is_ok() {
            self.pending += 1;
        }
    }

    pub fn is_busy(&self) -> bool {
        self.pending > 0
    }

    pub fn poll(&mut self) -> Vec<ImportResult> {
        let results: Vec<_> = self.results.try_iter().collect();
        self.pending = self.pending.saturating_sub(results.len());
        results
    }
}

fn probe(path: &std::path::Path, timebase: Timebase) -> Result<ImportMedia, MediaError> {
    let reader = MediaReader::open(path, timebase)?;
    let info = reader.info();
    let name = path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    Ok(ImportMedia {
        name,
        path: path.to_owned(),
        duration: info.duration.ok_or(MediaError::UnknownDuration)?,
        video: info.video.as_ref().map(|video| VideoFormat {
            width: video.width,
            height: video.height,
            frame_rate: video.frame_rate.unwrap_or(FALLBACK_RATE),
        }),
        has_audio: info.has_audio,
    })
}
