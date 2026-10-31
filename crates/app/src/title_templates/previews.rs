//! Pictures of title templates for the Graphics panel, drawn on a worker by the same
//! code that draws them in the program.

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::thread;

use crossbeam::channel::{self, Receiver, Sender};
use egui::{ColorImage, TextureHandle, TextureOptions};
use timeline::{FrameRate, Generator};

/// The longer side of a card's picture, in pixels, sharp on high-density displays.
pub(super) const PREVIEW_SIZE: u32 = 320;

struct Request {
    key: u64,
    generator: Generator,
    canvas: [u32; 2],
}

pub(super) struct Previews {
    requests: Sender<Request>,
    replies: Receiver<(u64, ColorImage)>,
    shown: HashMap<u64, TextureHandle>,
    asked: HashSet<u64>,
}

impl Previews {
    pub(super) fn start() -> std::io::Result<Self> {
        let (request_tx, request_rx) = channel::unbounded::<Request>();
        let (reply_tx, reply_rx) = channel::unbounded();
        thread::Builder::new()
            .name("template previews".to_owned())
            .spawn(move || {
                for request in request_rx {
                    let picture = drawn(&request.generator, request.canvas);
                    if reply_tx.send((request.key, picture)).is_err() {
                        return;
                    }
                }
            })?;
        Ok(Self {
            requests: request_tx,
            replies: reply_rx,
            shown: HashMap::new(),
            asked: HashSet::new(),
        })
    }

    /// The picture of `generator` drawn on a `canvas`-sized frame, asked for when not
    /// yet drawn.
    pub(super) fn picture(
        &mut self,
        generator: &Generator,
        canvas: [u32; 2],
    ) -> Option<&TextureHandle> {
        let key = key(generator, canvas);
        if !self.shown.contains_key(&key) && self.asked.insert(key) {
            let request = Request {
                key,
                generator: generator.clone(),
                canvas,
            };
            if self.requests.send(request).is_err() {
                self.asked.remove(&key);
            }
        }
        self.shown.get(&key)
    }

    /// Takes in drawn pictures; true when any came.
    pub(super) fn poll(&mut self, ctx: &egui::Context) -> bool {
        let mut came = false;
        for (key, picture) in self.replies.try_iter() {
            let texture = ctx.load_texture("template preview", picture, TextureOptions::LINEAR);
            self.shown.insert(key, texture);
            came = true;
        }
        came
    }
}

fn key(generator: &Generator, canvas: [u32; 2]) -> u64 {
    let mut hasher = DefaultHasher::new();
    (generator, canvas).hash(&mut hasher);
    hasher.finish()
}

/// `generator` on a `canvas`-sized frame, shrunk to the preview size. A roll or crawl
/// draws a strip longer than the frame; its middle stands for it.
fn drawn(generator: &Generator, [width, height]: [u32; 2]) -> ColorImage {
    let frame = playback::generated_frame(generator, (0, FrameRate::FPS_25), width, height);
    let left = frame.width.saturating_sub(width) / 2;
    let top = frame.height.saturating_sub(height) / 2;
    let (width, height) = (width.min(frame.width), height.min(frame.height));
    let step = (width.max(height) / PREVIEW_SIZE).max(1);
    let [shown_width, shown_height] = [width / step, height / step].map(|side| side.max(1));
    let mut pixels = Vec::with_capacity((shown_width * shown_height * 4) as usize);
    for y in 0..shown_height {
        for x in 0..shown_width {
            pixels.extend(averaged(&frame, [left + x * step, top + y * step], step));
        }
    }
    ColorImage::from_rgba_unmultiplied([shown_width as usize, shown_height as usize], &pixels)
}

/// The mean of the `step`-sized square of `frame` at `corner`, weighted by alpha so
/// the color of see-through pixels doesn't darken edges.
fn averaged(frame: &media::VideoFrame, [left, top]: [u32; 2], step: u32) -> [u8; 4] {
    let mut sum = [0u64; 4];
    for y in top..top + step {
        let row = (y * frame.width) as usize * 4;
        for x in left..left + step {
            let at = row + x as usize * 4;
            let Some(&[r, g, b, a]) = frame.pixels.get(at..at + 4) else {
                continue;
            };
            let alpha = u64::from(a);
            sum[0] += u64::from(r) * alpha;
            sum[1] += u64::from(g) * alpha;
            sum[2] += u64::from(b) * alpha;
            sum[3] += alpha;
        }
    }
    let count = u64::from(step * step);
    if sum[3] == 0 {
        return [0; 4];
    }
    let color = |channel: u64| (channel / sum[3]) as u8;
    [
        color(sum[0]),
        color(sum[1]),
        color(sum[2]),
        (sum[3] / count) as u8,
    ]
}
