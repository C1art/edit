use std::path::PathBuf;
use std::thread;

use crossbeam::channel::{self, Receiver, Sender};
use media::{MediaError, MediaReader};
use timeline::{held_length, FrameRate, ImportMedia, SourceId, Timebase, VideoFormat};

/// Rate assumed for video whose container declares none.
const FALLBACK_RATE: FrameRate = FrameRate::FPS_25;

/// A probed file, and the media item it is to replace rather than join the pool.
pub struct Probed {
    pub replacing: Option<SourceId>,
    pub result: Result<ImportMedia, (PathBuf, MediaError)>,
}

/// A file to probe, the item it replaces, and the rate an image sequence plays at.
type Request = (PathBuf, Option<SourceId>, FrameRate);

/// Probes files on a worker thread so the UI never touches the disk.
pub struct Importer {
    requests: Sender<Request>,
    results: Receiver<Probed>,
    pending: usize,
}

impl Importer {
    pub fn new(timebase: Timebase) -> std::io::Result<Self> {
        let (request_tx, request_rx) = channel::unbounded::<Request>();
        let (result_tx, result_rx) = channel::unbounded();
        thread::Builder::new()
            .name("import".to_owned())
            .spawn(move || {
                for (path, replacing, sequence_rate) in request_rx {
                    let result =
                        probe(&path, timebase, sequence_rate).map_err(|error| (path, error));
                    if result_tx.send(Probed { replacing, result }).is_err() {
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

    pub fn request(
        &mut self,
        path: PathBuf,
        replacing: Option<SourceId>,
        sequence_rate: FrameRate,
    ) {
        if self.requests.send((path, replacing, sequence_rate)).is_ok() {
            self.pending += 1;
        }
    }

    pub fn is_busy(&self) -> bool {
        self.pending > 0
    }

    pub fn poll(&mut self) -> Vec<Probed> {
        let results: Vec<_> = self.results.try_iter().collect();
        self.pending = self.pending.saturating_sub(results.len());
        results
    }
}

/// Reads the file as an import would; an image sequence plays at `sequence_rate`, as
/// pictures declare none of their own.
pub(crate) fn probe(
    path: &std::path::Path,
    timebase: Timebase,
    sequence_rate: FrameRate,
) -> Result<ImportMedia, MediaError> {
    let reader = MediaReader::open(path, timebase)?;
    let info = reader.info();
    let first_frame = info
        .sequence
        .then(|| media::source_files(path).into_iter().next());
    let named = first_frame.flatten().unwrap_or_else(|| path.to_owned());
    let name = named.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    Ok(ImportMedia {
        name,
        path: path.to_owned(),
        duration: match info.duration {
            _ if info.still => held_length(timebase),
            Some(duration) => duration,
            None => return Err(MediaError::UnknownDuration),
        },
        video: info.video.as_ref().map(|video| VideoFormat {
            width: video.width,
            height: video.height,
            frame_rate: video.frame_rate.unwrap_or(FALLBACK_RATE),
            pixel_aspect: video.pixel_aspect,
        }),
        has_audio: info.has_audio,
        still: info.still,
        interpreted_rate: info.sequence.then_some(sequence_rate),
    })
}
