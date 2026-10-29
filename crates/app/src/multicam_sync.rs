use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam::channel::{self, Receiver, TryRecvError};
use media::Peaks;
use timeline::{SourceId, Ticks, Timebase};

use crate::editor::Editor;

/// How the cameras of a new multi-camera source are lined up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MulticamSync {
    /// All cameras start together.
    Start,
    /// Each camera starts where its sound matches the first camera's.
    Audio,
}

/// What synchronized sources are made into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Synced {
    Multicam,
    /// A merged clip of the first source's picture and every source's sound.
    Merge {
        remove_camera_audio: bool,
    },
}

/// Sources waiting for their sound to be read and matched.
pub struct SyncJob {
    sources: Vec<SourceId>,
    synced: Synced,
    matching: Option<Receiver<Vec<Option<Ticks>>>>,
}

impl Editor {
    pub(crate) fn sync_sources(
        &mut self,
        sources: Vec<SourceId>,
        sync: MulticamSync,
        synced: Synced,
    ) {
        match sync {
            MulticamSync::Start => self.create_synced(synced, sources, Vec::new()),
            MulticamSync::Audio => {
                self.notice("Synchronizing the clips by their sound…");
                self.multicam_sync = Some(SyncJob {
                    sources,
                    synced,
                    matching: None,
                });
            }
        }
    }

    /// Advances an audio sync: once every camera's peaks are read, they are matched on a
    /// worker, and once matched the source is made.
    pub(crate) fn poll_multicam_sync(&mut self, ctx: &egui::Context) {
        let Some(job) = &mut self.multicam_sync else {
            return;
        };
        ctx.request_repaint_after(Duration::from_millis(100));
        let Some(matching) = &job.matching else {
            let pool = self.timeline.project();
            let paths: Vec<_> = job
                .sources
                .iter()
                .map(|&id| {
                    pool.media_item(id)
                        .filter(|item| item.has_audio)
                        .map(|item| item.path.clone())
                })
                .collect();
            let timebase = pool.timebase;
            let mut peaks = Vec::with_capacity(paths.len());
            for path in paths {
                match path {
                    Some(path) => match self.clip_art.shared_peaks(&path) {
                        Some(read) => peaks.push(read),
                        None => return,
                    },
                    None => peaks.push(None),
                }
            }
            let (reply, matched) = channel::bounded(1);
            let spawned = thread::Builder::new()
                .name("multicam sync".to_owned())
                .spawn(move || {
                    let _ = reply.send(match_cameras(&peaks, timebase));
                });
            match spawned {
                Ok(_) => job.matching = Some(matched),
                Err(error) => {
                    self.multicam_sync = None;
                    self.notice(format!("Cannot synchronize the clips: {error}"));
                }
            }
            return;
        };
        let starts = match matching.try_recv() {
            Ok(starts) => starts,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => {
                self.multicam_sync = None;
                return;
            }
        };
        let Some(job) = self.multicam_sync.take() else {
            return;
        };
        let unmatched = starts.iter().filter(|start| start.is_none()).count();
        let starts = starts.into_iter().map(Option::unwrap_or_default).collect();
        self.create_synced(job.synced, job.sources, starts);
        if unmatched > 0 {
            self.notice(format!(
                "{unmatched} clip(s) had no sound matching the first and start with it"
            ));
        }
    }

    fn create_synced(&mut self, synced: Synced, sources: Vec<SourceId>, starts: Vec<Ticks>) {
        match synced {
            Synced::Multicam => self.create_multicam(sources, starts),
            Synced::Merge {
                remove_camera_audio,
            } => self.create_merged(sources, starts, remove_camera_audio),
        }
    }
}

/// Where each camera starts against the first one with sound, or None when its sound
/// doesn't match that camera's.
fn match_cameras(peaks: &[Option<Arc<Peaks>>], timebase: Timebase) -> Vec<Option<Ticks>> {
    let reference = peaks.iter().flatten().next();
    peaks
        .iter()
        .map(|camera| {
            let (reference, camera) = (reference?, camera.as_ref()?);
            if Arc::ptr_eq(reference, camera) {
                return Some(Ticks::ZERO);
            }
            reference.offset_of(camera, timebase)
        })
        .collect()
}
