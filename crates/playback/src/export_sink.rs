use std::path::PathBuf;
use std::thread::{self, JoinHandle};

use crossbeam::channel::{self, Sender};
use media::{MediaWriter, RgbaResizer, StillFormat};

use crate::PlaybackError;

/// What the export renders, in the order it goes into the file.
pub(crate) enum Work {
    Picture(Vec<u8>),
    Sound(Vec<f32>),
    Still(PathBuf, Vec<u8>),
    Finish,
}

/// Where the pictures of a still sequence go and how big.
#[derive(Clone, Copy)]
pub(crate) struct Stills {
    pub format: StillFormat,
    pub input_size: (u32, u32),
    pub size: (u32, u32),
}

/// Encodes and writes on a thread of its own, so the encoder works on one frame while
/// the next is decoded and composited. Dropping it without `Finish` abandons the file.
pub(crate) struct Sink {
    work: Option<Sender<Work>>,
    thread: Option<JoinHandle<Result<(), PlaybackError>>>,
}

/// Frames are tens of megabytes at 4K; a few cover the jitter between the two sides.
const QUEUED: usize = 3;

impl Sink {
    pub fn start(
        writer: Option<MediaWriter>,
        stills: Option<Stills>,
    ) -> Result<Self, PlaybackError> {
        let (work, queued) = channel::bounded(QUEUED);
        let thread = thread::Builder::new()
            .name("export writer".into())
            .spawn(move || {
                let mut writer = writer;
                let mut resizer = RgbaResizer::default();
                for work in queued {
                    match work {
                        Work::Picture(picture) => {
                            if let Some(writer) = &mut writer {
                                writer.write_video(&picture)?;
                            }
                        }
                        Work::Sound(samples) => {
                            if let Some(writer) = &mut writer {
                                writer.write_audio(&samples)?;
                            }
                        }
                        Work::Still(path, picture) => {
                            if let Some(stills) = stills {
                                let picture = if stills.size == stills.input_size {
                                    picture
                                } else {
                                    resizer.resize(stills.input_size, stills.size, &picture)?
                                };
                                media::write_still(&path, stills.format, stills.size, &picture)?;
                            }
                        }
                        Work::Finish => {
                            if let Some(writer) = writer.take() {
                                writer.finish()?;
                            }
                            break;
                        }
                    }
                }
                Ok(())
            })
            .map_err(PlaybackError::WorkerSpawn)?;
        Ok(Self {
            work: Some(work),
            thread: Some(thread),
        })
    }

    /// Queues `work`, waiting while the writer is behind. A writer that stopped hands
    /// back why.
    pub fn send(&mut self, work: Work) -> Result<(), PlaybackError> {
        let sent = self
            .work
            .as_ref()
            .is_some_and(|sender| sender.send(work).is_ok());
        if sent {
            Ok(())
        } else {
            self.stop()
        }
    }

    /// Writes what's left and closes the file.
    pub fn finish(mut self) -> Result<(), PlaybackError> {
        self.send(Work::Finish)?;
        self.stop()
    }

    fn stop(&mut self) -> Result<(), PlaybackError> {
        self.work = None;
        match self.thread.take().map(JoinHandle::join) {
            Some(Ok(result)) => result,
            Some(Err(panic)) => std::panic::resume_unwind(panic),
            None => Ok(()),
        }
    }
}

impl Drop for Sink {
    /// Waits for the file to close, so a cancelled export's partial file can be removed.
    fn drop(&mut self) {
        self.work = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
