use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use audio::{MusicFeatures, SampleRate};
use crossbeam::channel::{Receiver, Sender};
use timeline::{ClipId, Sequence, Ticks};

use crate::loudness::{clip_alone, Piece};
use crate::sequence_mix::{SequenceMix, CHUNK_FRAMES};
use crate::PlaybackError;

const LISTEN_RATE: SampleRate = SampleRate(48_000);

#[derive(Debug)]
pub enum RemixEvent {
    /// The fraction of the music listened to so far.
    Progress(f32),
    /// Part of the media could not be read and was heard as silence.
    Warning(PlaybackError),
    /// The pieces of the source, in source time, to lay the clip out as.
    Planned(Vec<Range<Ticks>>),
}

/// Listens to the whole source of a music clip on a worker thread and plans how to
/// cut or loop it to a new length. Dropping the job cancels it.
pub struct RemixJob {
    events: Receiver<RemixEvent>,
    cancel: Arc<AtomicBool>,
}

impl RemixJob {
    /// `None` when `clip` is not in `sequence`.
    pub fn start(
        sequence: &Sequence,
        clip: ClipId,
        target: Ticks,
    ) -> Result<Option<Self>, PlaybackError> {
        let Some(source) = whole_source(sequence, clip) else {
            return Ok(None);
        };
        let (sender, events) = crossbeam::channel::unbounded();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        thread::Builder::new()
            .name("remix".into())
            .spawn(move || listen(&source, target, &sender, &worker_cancel))
            .map_err(PlaybackError::WorkerSpawn)?;
        Ok(Some(Self { events, cancel }))
    }

    /// Everything that happened since the last poll; never blocks.
    pub fn poll(&self) -> Vec<RemixEvent> {
        self.events.try_iter().collect()
    }
}

impl Drop for RemixJob {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// `clip` alone, stretched over all of its source, placed so that sequence time is
/// source time.
fn whole_source(sequence: &Sequence, clip: ClipId) -> Option<Piece> {
    let (_, found) = sequence.clip(clip)?;
    let media = sequence.media.iter().find(|item| item.id == found.source)?;
    let span = media.subclip.clone().unwrap_or(Ticks(0)..media.duration);
    let (mut alone, _) = clip_alone(sequence, clip)?;
    let kept = Arc::make_mut(&mut alone)
        .audio
        .iter_mut()
        .flat_map(|track| &mut track.clips)
        .find(|kept| kept.id == clip)
        .map(Arc::make_mut)?;
    kept.start = span.start;
    kept.source_in = span.start;
    kept.duration = span.end - span.start;
    Some((alone, span))
}

fn listen(source: &Piece, target: Ticks, events: &Sender<RemixEvent>, cancel: &AtomicBool) {
    let (sequence, span) = source;
    let mut mix = SequenceMix::new(LISTEN_RATE);
    let rate = mix.rate();
    let to_frames = |time: Ticks| time.rescale(sequence.timebase, rate).0;
    let frames = to_frames(span.start)..to_frames(span.end);
    let total = (frames.end - frames.start).max(1);
    let mut features = MusicFeatures::new(LISTEN_RATE);
    let mut mono = Vec::with_capacity(CHUNK_FRAMES);
    let mut warnings = Vec::new();
    for start in frames.clone().step_by(CHUNK_FRAMES) {
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        mix.mix_chunk(sequence, start, &mut warnings);
        let length = usize::try_from((frames.end - start).min(CHUNK_FRAMES as i64)).unwrap_or(0);
        mono.clear();
        mono.extend(
            mix.mix[..length * 2]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|[left, right]| (left + right) * 0.5),
        );
        features.push(&mono);
        for warning in warnings.drain(..) {
            let _ = events.send(RemixEvent::Warning(warning));
        }
        let done = start + length as i64 - frames.start;
        let _ = events.send(RemixEvent::Progress(done as f32 / total as f32));
    }
    let wanted = u64::try_from(to_frames(target)).unwrap_or(0);
    let to_ticks = |samples: u64| {
        let samples = Ticks(i64::try_from(samples).unwrap_or(i64::MAX));
        span.start + samples.rescale(rate, sequence.timebase)
    };
    let pieces = features
        .plan(wanted)
        .into_iter()
        .map(|piece| to_ticks(piece.start)..to_ticks(piece.end).min(span.end))
        .filter(|piece| piece.start < piece.end)
        .collect();
    let _ = events.send(RemixEvent::Planned(pieces));
}
