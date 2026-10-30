use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use crossbeam::channel::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use media::{DecodedFrame, MediaReader, Streams, VideoFrame};
use timeline::{Ticks, Timebase};

/// Sharp on high-density displays at the timeline's row height.
const THUMBNAIL_HEIGHT: u32 = 72;
/// Open files kept for further requests; most timelines show a handful of sources.
pub(crate) const OPEN_READERS: usize = 6;
/// Open files are closed after this long without requests, as each decoder holds tens
/// of megabytes of frames and threads.
const IDLE_CLOSE: Duration = Duration::from_secs(3);
/// Requests beyond this are the oldest, so they're likely scrolled away; they're handed
/// back unserved and asked for again if still in view.
const MAX_QUEUED: usize = 96;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub path: PathBuf,
    pub at: Ticks,
    pub timebase: Timebase,
}

pub enum Reply {
    Frame(Request, VideoFrame),
    Failed(Request),
    Dropped(Request),
}

/// Decodes small pictures for timeline clips on a worker, in the order that decodes least.
pub struct ThumbnailWorker {
    requests: Sender<Request>,
    replies: Receiver<Reply>,
}

impl ThumbnailWorker {
    pub fn new() -> std::io::Result<Self> {
        let (request_tx, request_rx) = channel::unbounded();
        let (reply_tx, reply_rx) = channel::unbounded();
        thread::Builder::new()
            .name("thumbnails".to_owned())
            .spawn(move || run(&request_rx, &reply_tx))?;
        Ok(Self {
            requests: request_tx,
            replies: reply_rx,
        })
    }

    /// False once the worker has stopped.
    pub fn request(&self, request: Request) -> bool {
        self.requests.send(request).is_ok()
    }

    pub fn poll(&self) -> impl Iterator<Item = Reply> + '_ {
        self.replies.try_iter()
    }
}

fn run(requests: &Receiver<Request>, replies: &Sender<Reply>) {
    let mut queue = VecDeque::new();
    let mut readers: HashMap<PathBuf, MediaReader> = HashMap::new();
    loop {
        if queue.is_empty() {
            let request = if readers.is_empty() {
                requests.recv().map_err(|_| RecvTimeoutError::Disconnected)
            } else {
                requests.recv_timeout(IDLE_CLOSE)
            };
            match request {
                Ok(request) => queue.push_back(request),
                Err(RecvTimeoutError::Timeout) => {
                    readers.clear();
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
        loop {
            match requests.try_recv() {
                Ok(request) => queue.push_back(request),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }
        while queue.len() > MAX_QUEUED {
            if let Some(stale) = queue.pop_front() {
                if replies.send(Reply::Dropped(stale)).is_err() {
                    return;
                }
            }
        }
        let Some(request) = take_next(&mut queue, &readers) else {
            continue;
        };
        let reply = match decode(&mut readers, &request) {
            Some(frame) => Reply::Frame(request, frame),
            None => Reply::Failed(request),
        };
        if replies.send(reply).is_err() {
            return;
        }
    }
}

/// The request that costs least to serve next: one an open file reaches by decoding on,
/// earliest first so the others stay ahead of it. Otherwise the earliest one in the file
/// asked about last, which the rest of that file's requests can then follow on from.
fn take_next(
    queue: &mut VecDeque<Request>,
    readers: &HashMap<PathBuf, MediaReader>,
) -> Option<Request> {
    let reachable = |request: &Request| {
        readers
            .get(&request.path)
            .is_some_and(|reader| reader.reaches_without_seek(request.at))
    };
    let newest = &queue.back()?.path;
    let index = queue
        .iter()
        .enumerate()
        .filter(|(_, request)| reachable(request))
        .min_by_key(|(_, request)| request.at)
        .or_else(|| {
            queue
                .iter()
                .enumerate()
                .filter(|(_, request)| request.path == *newest)
                .min_by_key(|(_, request)| request.at)
        })
        .map(|(index, _)| index)?;
    queue.remove(index)
}

fn decode(readers: &mut HashMap<PathBuf, MediaReader>, request: &Request) -> Option<VideoFrame> {
    if !readers.contains_key(&request.path) {
        if readers.len() >= OPEN_READERS {
            readers.clear();
        }
        let reader = open(request)?;
        readers.insert(request.path.clone(), reader);
    }
    let reader = readers.get_mut(&request.path)?;
    let frame = first_picture(reader, request.at);
    if frame.is_none() {
        // A reader that failed mid-stream may be stuck; reopen it next time.
        readers.remove(&request.path);
    }
    frame
}

fn open(request: &Request) -> Option<MediaReader> {
    let mut reader =
        MediaReader::open_streams(&request.path, request.timebase, Streams::Video).ok()?;
    let video = reader.info().video.clone()?;
    let size = media::proxy_size((video.width, video.height), THUMBNAIL_HEIGHT);
    reader.set_video_size(Some(size)).ok()?;
    Some(reader)
}

fn first_picture(reader: &mut MediaReader, at: Ticks) -> Option<VideoFrame> {
    reader.advance(at).ok()?;
    while let Some(frame) = reader.next_frame().ok()? {
        if let DecodedFrame::Video(picture) = frame {
            return Some(picture);
        }
    }
    None
}
