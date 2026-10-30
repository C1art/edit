use std::collections::BTreeMap;
use std::path::PathBuf;
use std::thread;

use crossbeam::channel::{self, Receiver, Sender};
use media::{MediaError, MediaReader};
use timeline::{held_length, FrameRate, ImportMedia, SourceId, Timebase, VideoFormat};

/// Rate assumed for video whose container declares none.
const FALLBACK_RATE: FrameRate = FrameRate::FPS_25;
/// Files probed at once; probing mostly waits on the disk.
const PROBES_AT_ONCE: usize = 4;

/// A probed file, and the media item it is to replace rather than join the pool.
pub struct Probed {
    pub replacing: Option<SourceId>,
    pub media: ImportMedia,
}

type Outcome = (Option<SourceId>, Result<ImportMedia, (PathBuf, MediaError)>);

/// A file to probe, the item it replaces, and the rate an image sequence plays at.
type Request = (PathBuf, Option<SourceId>, FrameRate);

/// Probes files on worker threads so the UI never touches the disk.
pub struct Importer {
    requests: Sender<(u64, Request)>,
    results: Receiver<(u64, Outcome)>,
    /// Results that came in ahead of earlier requests, held back so files join the pool
    /// in the order they were asked for.
    early: BTreeMap<u64, Outcome>,
    /// Files that couldn't be read, held until the rest of their batch is in so that
    /// they're reported together.
    failed: Vec<(PathBuf, MediaError)>,
    requested: u64,
    handed_out: u64,
}

impl Importer {
    pub fn new(timebase: Timebase) -> std::io::Result<Self> {
        let (request_tx, request_rx) = channel::unbounded::<(u64, Request)>();
        let (result_tx, result_rx) = channel::unbounded();
        for index in 0..PROBES_AT_ONCE {
            let (requests, results) = (request_rx.clone(), result_tx.clone());
            let spawned = thread::Builder::new()
                .name(format!("import {index}"))
                .spawn(move || {
                    for (number, (path, replacing, sequence_rate)) in requests {
                        let result =
                            probe(&path, timebase, sequence_rate).map_err(|error| (path, error));
                        if results.send((number, (replacing, result))).is_err() {
                            break;
                        }
                    }
                });
            // One worker is enough to import; more only make it quicker.
            if let Err(error) = spawned {
                if index == 0 {
                    return Err(error);
                }
                break;
            }
        }
        Ok(Self {
            requests: request_tx,
            results: result_rx,
            early: BTreeMap::new(),
            failed: Vec::new(),
            requested: 0,
            handed_out: 0,
        })
    }

    pub fn request(
        &mut self,
        path: PathBuf,
        replacing: Option<SourceId>,
        sequence_rate: FrameRate,
    ) {
        let request = (path, replacing, sequence_rate);
        if self.requests.send((self.requested, request)).is_ok() {
            self.requested += 1;
        }
    }

    pub fn is_busy(&self) -> bool {
        self.handed_out < self.requested
    }

    /// Files asked for that haven't joined the pool yet.
    pub fn remaining(&self) -> u64 {
        self.requested - self.handed_out
    }

    pub fn poll(&mut self) -> Vec<Probed> {
        self.early.extend(self.results.try_iter());
        let mut ready = Vec::new();
        while let Some((replacing, result)) = self.early.remove(&self.handed_out) {
            match result {
                Ok(media) => ready.push(Probed { replacing, media }),
                Err(failure) => self.failed.push(failure),
            }
            self.handed_out += 1;
        }
        ready
    }

    /// The files that couldn't be read, once nothing more is being imported.
    pub fn take_failures(&mut self) -> Vec<(PathBuf, MediaError)> {
        if self.is_busy() {
            return Vec::new();
        }
        std::mem::take(&mut self.failed)
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
            color_space: video.color_space,
            field_order: video.field_order,
        }),
        has_audio: info.has_audio,
        still: info.still,
        interpreted_rate: info.sequence.then_some(sequence_rate),
        placed: None,
    })
}
