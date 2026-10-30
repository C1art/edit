//! Synchronize: the selected timeline clips lined up with the earliest of them, by
//! where they start or end, or by matching the sound they recorded together.

use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam::channel::{self, Receiver, TryRecvError};
use media::Peaks;
use timeline::{synchronized_clips, ClipId, Sequence, SynchronizeClips, Ticks, Timebase};

use crate::editor::Editor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SyncPoint {
    ClipStart,
    ClipEnd,
    Audio,
}

impl SyncPoint {
    pub(crate) const ALL: [Self; 3] = [Self::ClipStart, Self::ClipEnd, Self::Audio];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::ClipStart => "Clip Start",
            Self::ClipEnd => "Clip End",
            Self::Audio => "Audio",
        }
    }
}

/// Clips waiting for their sound to be read and matched against the first's.
pub(crate) struct ClipSyncJob {
    clips: Vec<ClipId>,
    matching: Option<Receiver<Vec<Option<Ticks>>>>,
}

impl Editor {
    pub(crate) fn synchronize(&mut self, point: SyncPoint) {
        let sequence = self.timeline.sequence();
        let clips = synchronized_clips(sequence, &self.view.selection);
        let Some((&anchor, others)) = clips.split_first().filter(|(_, rest)| !rest.is_empty())
        else {
            self.notice("Select clips of two or more recordings to synchronize");
            return;
        };
        let Some((_, anchor)) = sequence.clip(anchor) else {
            return;
        };
        let starts: Vec<_> = others
            .iter()
            .filter_map(|&id| {
                let (_, clip) = sequence.clip(id)?;
                let start = match point {
                    SyncPoint::ClipStart => anchor.start,
                    SyncPoint::ClipEnd => anchor.end() - clip.duration,
                    SyncPoint::Audio => return None,
                };
                Some((id, start))
            })
            .collect();
        if point == SyncPoint::Audio {
            self.notice("Synchronizing the clips by their sound…");
            self.clip_sync = Some(ClipSyncJob {
                clips,
                matching: None,
            });
        } else {
            self.apply(SynchronizeClips { starts });
        }
    }

    /// Reads each clip's sound, matches it against the first clip's on a worker, then
    /// moves the clips to where they match.
    pub(crate) fn poll_clip_sync(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.clip_sync else {
            return;
        };
        ctx.request_repaint_after(Duration::from_millis(100));
        let Some(matching) = &job.matching else {
            self.start_matching();
            return;
        };
        let offsets = match matching.try_recv() {
            Ok(offsets) => offsets,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => {
                self.clip_sync = None;
                return;
            }
        };
        let Some(job) = self.clip_sync.take() else {
            return;
        };
        let sequence = self.timeline.sequence();
        let starts = matched_starts(sequence, &job.clips, &offsets);
        let unmatched = offsets.iter().filter(|offset| offset.is_none()).count();
        if !starts.is_empty() {
            self.apply(SynchronizeClips { starts });
        }
        if unmatched > 0 {
            self.notice(format!(
                "{unmatched} clip(s) had no sound matching the first and stayed put"
            ));
        }
    }

    fn start_matching(&mut self) {
        let Some(job) = &self.clip_sync else {
            return;
        };
        let sequence = self.timeline.sequence();
        let paths: Vec<Option<PathBuf>> = job
            .clips
            .iter()
            .map(|&id| {
                let (_, clip) = sequence.clip(id)?;
                let item = sequence.media_item(clip.source)?;
                (item.has_audio && item.is_file()).then(|| item.path.clone())
            })
            .collect();
        let timebase = sequence.timebase;
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
            .name("clip sync".to_owned())
            .spawn(move || {
                let _ = reply.send(match_sound(&peaks, timebase));
            });
        match spawned {
            Ok(_) => {
                if let Some(job) = &mut self.clip_sync {
                    job.matching = Some(matched);
                }
            }
            Err(error) => {
                self.clip_sync = None;
                self.notice(format!("Cannot synchronize the clips: {error}"));
            }
        }
    }
}

/// Where each clip after the first starts in its file's time against the first's file,
/// or None when its sound doesn't match.
fn match_sound(peaks: &[Option<Arc<Peaks>>], timebase: Timebase) -> Vec<Option<Ticks>> {
    let Some((anchor, others)) = peaks.split_first() else {
        return Vec::new();
    };
    others
        .iter()
        .map(|other| {
            let (anchor, other) = (anchor.as_ref()?, other.as_ref()?);
            if Arc::ptr_eq(anchor, other) {
                return Some(Ticks::ZERO);
            }
            anchor.offset_of(other, timebase)
        })
        .collect()
}

/// The start each matched clip takes so the moment its sound shares with the first clip
/// plays at the same time as there.
fn matched_starts(
    sequence: &Sequence,
    clips: &[ClipId],
    offsets: &[Option<Ticks>],
) -> Vec<(ClipId, Ticks)> {
    let Some((anchor, others)) = clips.split_first() else {
        return Vec::new();
    };
    let Some((_, anchor)) = sequence.clip(*anchor) else {
        return Vec::new();
    };
    others
        .iter()
        .zip(offsets)
        .filter_map(|(&id, offset)| {
            let (_, clip) = sequence.clip(id)?;
            Some((id, anchor.position_of(clip.source_in + (*offset)?)))
        })
        .collect()
}
