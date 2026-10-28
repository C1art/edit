mod thumbnails;
mod waveforms;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::{ColorImage, TextureHandle, TextureId, TextureOptions};
use media::{Peaks, VideoFrame};
use timeline::{Ticks, Timebase};

pub use thumbnails::ThumbnailWorker;
use thumbnails::{Reply, Request};
pub use waveforms::WaveformWorker;

/// Pictures kept on the GPU; at 128×72 this is about 18 MB.
const MAX_PICTURES: usize = 480;

enum PeakState {
    Reading,
    Ready(Arc<Peaks>),
    Failed,
}

enum Picture {
    Pending,
    Ready {
        texture: TextureHandle,
        /// The frame it was last drawn in, to evict the least recently seen.
        used: u64,
    },
    Failed,
}

/// Waveforms and thumbnails for timeline clips, made on workers and cached by file.
pub struct ClipArt {
    waveforms: Option<WaveformWorker>,
    peaks: HashMap<PathBuf, PeakState>,
    thumbnails: Option<ThumbnailWorker>,
    pictures: HashMap<PathBuf, BTreeMap<Ticks, Picture>>,
    /// Pictures are keyed by time in this timebase.
    timebase: Timebase,
    frame: u64,
    pub show_waveforms: bool,
    pub show_thumbnails: bool,
}

impl ClipArt {
    pub fn new(waveforms: Option<WaveformWorker>, thumbnails: Option<ThumbnailWorker>) -> Self {
        Self {
            waveforms,
            peaks: HashMap::new(),
            thumbnails,
            pictures: HashMap::new(),
            timebase: Timebase::HZ_90K,
            frame: 0,
            show_waveforms: true,
            show_thumbnails: true,
        }
    }

    /// Takes in finished work; call once per frame before drawing the timeline.
    pub fn poll(&mut self, ctx: &egui::Context) {
        self.frame += 1;
        if let Some(worker) = &self.waveforms {
            for (path, peaks) in worker.poll() {
                let state =
                    peaks.map_or(PeakState::Failed, |peaks| PeakState::Ready(Arc::new(peaks)));
                self.peaks.insert(path, state);
            }
        }
        let replies: Vec<Reply> = self
            .thumbnails
            .as_ref()
            .map(|worker| worker.poll().collect())
            .unwrap_or_default();
        for reply in replies {
            self.receive(ctx, reply);
        }
        self.evict();
        let reading = self
            .peaks
            .values()
            .any(|state| matches!(state, PeakState::Reading));
        let decoding = self
            .pictures
            .values()
            .flat_map(BTreeMap::values)
            .any(|picture| matches!(picture, Picture::Pending));
        if reading || decoding {
            ctx.request_repaint();
        }
    }

    fn receive(&mut self, ctx: &egui::Context, reply: Reply) {
        let (request, picture) = match reply {
            Reply::Frame(request, frame) => {
                let texture = upload(ctx, &request, &frame);
                let used = self.frame;
                (request, Some(Picture::Ready { texture, used }))
            }
            Reply::Failed(request) => (request, Some(Picture::Failed)),
            // Forgotten so it's asked for again if still in view.
            Reply::Dropped(request) => (request, None),
        };
        if request.timebase != self.timebase {
            return;
        }
        let times = self.pictures.entry(request.path).or_default();
        match picture {
            Some(picture) => times.insert(request.at, picture),
            None => times.remove(&request.at),
        };
    }

    fn evict(&mut self) {
        let mut ready: Vec<(u64, PathBuf, Ticks)> = self
            .pictures
            .iter()
            .flat_map(|(path, times)| {
                times
                    .iter()
                    .filter_map(move |(&at, picture)| match picture {
                        Picture::Ready { used, .. } => Some((*used, path.clone(), at)),
                        _ => None,
                    })
            })
            .collect();
        if ready.len() <= MAX_PICTURES {
            return;
        }
        ready.sort_unstable_by_key(|(used, _, _)| *used);
        let excess = ready.len() - MAX_PICTURES * 3 / 4;
        for (_, path, at) in ready.into_iter().take(excess) {
            if let Some(times) = self.pictures.get_mut(&path) {
                times.remove(&at);
            }
        }
    }

    /// The audio peaks of `path`, starting to read them if they haven't been.
    pub fn peaks(&mut self, path: &Path) -> Option<&Peaks> {
        match self.peak_state(path) {
            PeakState::Ready(peaks) => Some(peaks),
            _ => None,
        }
    }

    /// The audio peaks of `path` to hand to another thread: None while they are read,
    /// then Some(None) for a file without readable sound.
    pub fn shared_peaks(&mut self, path: &Path) -> Option<Option<Arc<Peaks>>> {
        match self.peak_state(path) {
            PeakState::Reading => None,
            PeakState::Ready(peaks) => Some(Some(Arc::clone(peaks))),
            PeakState::Failed => Some(None),
        }
    }

    fn peak_state(&mut self, path: &Path) -> &PeakState {
        self.peaks
            .entry(path.to_owned())
            .or_insert_with(|| match &self.waveforms {
                Some(worker) if worker.request(path.to_owned()) => PeakState::Reading,
                _ => PeakState::Failed,
            })
    }

    /// The picture of `path` at media time `at`, asking for it if needed. Until it
    /// arrives, the nearest picture already made stands in.
    pub fn picture(&mut self, path: &Path, at: Ticks, timebase: Timebase) -> Option<TextureId> {
        if timebase != self.timebase {
            self.pictures.clear();
            self.timebase = timebase;
        }
        if !self
            .pictures
            .get(path)
            .is_some_and(|times| times.contains_key(&at))
        {
            let request = Request {
                path: path.to_owned(),
                at,
                timebase,
            };
            let picture = match &self.thumbnails {
                Some(worker) if worker.request(request) => Picture::Pending,
                _ => Picture::Failed,
            };
            self.pictures
                .entry(path.to_owned())
                .or_default()
                .insert(at, picture);
        }
        let frame = self.frame;
        let times = self.pictures.get_mut(path)?;
        let ready =
            |(_, picture): &(&Ticks, &mut Picture)| matches!(picture, Picture::Ready { .. });
        let before = times
            .range_mut(..=at)
            .rev()
            .find(ready)
            .map(|(&time, _)| time);
        let after = || times.range_mut(at..).find(ready).map(|(&time, _)| time);
        let time = before.or_else(after)?;
        match times.get_mut(&time) {
            Some(Picture::Ready { texture, used }) => {
                *used = frame;
                Some(texture.id())
            }
            _ => None,
        }
    }
}

fn upload(ctx: &egui::Context, request: &Request, frame: &VideoFrame) -> TextureHandle {
    let size = [frame.width as usize, frame.height as usize];
    let image = ColorImage::from_rgba_unmultiplied(size, &frame.pixels);
    let name = format!("thumbnail {} {}", request.path.display(), request.at.0);
    ctx.load_texture(name, image, TextureOptions::LINEAR)
}
