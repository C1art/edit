//! The costly segments and their fingerprints, worked out on a worker: writing out
//! what each shows takes seconds on a long sequence, too long to hold up a frame.

use std::ops::Range;
use std::thread;

use crossbeam::channel::{self, Receiver, Sender};
use timeline::{render_segments, segment_content, Sequence, Ticks};

/// A segment to render, named by the fingerprint of what it shows.
pub(super) type Segment = (Range<Ticks>, u64);

/// Which timeline a revision belongs to, then the revision: a replaced timeline starts
/// its revisions again.
type Asked = (u64, u64);

struct Request {
    asked: Asked,
    sequence: Sequence,
    ctx: egui::Context,
}

pub(super) struct SegmentWorker {
    requests: Sender<Request>,
    replies: Receiver<(Asked, Vec<Segment>)>,
    timeline: u64,
}

impl SegmentWorker {
    pub(super) fn start() -> std::io::Result<Self> {
        let (request_tx, request_rx) = channel::unbounded::<Request>();
        let (reply_tx, reply_rx) = channel::unbounded();
        thread::Builder::new()
            .name("render segments".to_owned())
            .spawn(move || {
                while let Ok(first) = request_rx.recv() {
                    // Only the newest revision matters once edits come faster than this.
                    let request = request_rx.try_iter().last().unwrap_or(first);
                    let segments = costly_segments(&request.sequence);
                    if reply_tx.send((request.asked, segments)).is_err() {
                        return;
                    }
                    request.ctx.request_repaint();
                }
            })?;
        Ok(Self {
            requests: request_tx,
            replies: reply_rx,
            timeline: 0,
        })
    }

    /// Ignores what's still being worked out for the timeline being replaced.
    pub(super) fn forget(&mut self) {
        self.timeline += 1;
    }

    pub(super) fn ask(&self, revision: u64, sequence: Sequence, ctx: &egui::Context) {
        let _ = self.requests.send(Request {
            asked: (self.timeline, revision),
            sequence,
            ctx: ctx.clone(),
        });
    }

    /// The newest segments worked out, with the revision they're of.
    pub(super) fn poll(&self) -> Option<(u64, Vec<Segment>)> {
        self.replies
            .try_iter()
            .filter(|((timeline, _), _)| *timeline == self.timeline)
            .last()
            .map(|((_, revision), segments)| (revision, segments))
    }
}

fn costly_segments(sequence: &Sequence) -> Vec<Segment> {
    render_segments(sequence)
        .into_iter()
        .filter_map(|range| Some((range.clone(), fingerprint(sequence, &range)?)))
        .collect()
}

/// What a segment shows, as a number; `None` when it can't be written out.
pub(super) fn fingerprint(sequence: &Sequence, range: &Range<Ticks>) -> Option<u64> {
    let content = segment_content(sequence, range);
    project::fingerprint(&(content, range.start, range.end)).ok()
}
