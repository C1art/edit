use std::ops::Range;
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::thread::{self, JoinHandle};

use crate::ffi::{Encoder, Frame, Packet, PacketTimebase, Received};
use crate::MediaError;

const STEREO: usize = 2;

/// Stretches of sound queued for the encoder. The muxer holds pictures back until the
/// sound beside them arrives, so a short queue keeps that small.
const QUEUED: usize = 8;

/// Encodes sound on a thread of its own, alongside the pictures instead of in turn
/// with them. Its packets come back in order, for the muxer.
pub(crate) struct AudioEncoding {
    samples: Option<SyncSender<Vec<f32>>>,
    packets: Receiver<Result<Packet, MediaError>>,
    thread: Option<JoinHandle<()>>,
    pub(crate) index: usize,
    pub(crate) timebase: PacketTimebase,
}

struct AudioStream {
    encoder: Encoder,
    frame: Frame,
    frame_size: usize,
    /// Interleaved stereo waiting to fill a whole encoder frame.
    pending: Vec<f32>,
    next_pts: i64,
}

impl AudioEncoding {
    pub(crate) fn start(
        encoder: Encoder,
        index: usize,
        sample_rate: u32,
    ) -> Result<Self, MediaError> {
        // Encoders taking any size get about 20 ms per frame.
        let frame_size = encoder.frame_size().unwrap_or(sample_rate as usize / 50);
        let mut stream = AudioStream {
            frame: encoder.audio_frame(sample_rate, frame_size)?,
            encoder,
            frame_size,
            pending: Vec::with_capacity(frame_size * STEREO * 2),
            next_pts: 0,
        };
        let timebase = stream.encoder.packet_timebase();
        let (samples, queued) = mpsc::sync_channel(QUEUED);
        let (sender, packets) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("export audio".into())
            .spawn(move || {
                if let Err(error) = stream.run(&queued, &sender) {
                    let _ = sender.send(Err(error));
                }
            })
            .map_err(|error| MediaError::Thread(error.to_string()))?;
        Ok(Self {
            samples: Some(samples),
            packets,
            thread: Some(thread),
            index,
            timebase,
        })
    }

    /// Queues interleaved stereo, waiting while the encoder is behind.
    pub(crate) fn send(&mut self, stereo: Vec<f32>) -> Result<(), MediaError> {
        let sent = self
            .samples
            .as_ref()
            .is_some_and(|samples| samples.send(stereo).is_ok());
        if sent {
            Ok(())
        } else {
            Err(self.failure())
        }
    }

    /// The packets encoded so far, without waiting for more.
    pub(crate) fn ready(&self) -> impl Iterator<Item = Result<Packet, MediaError>> + '_ {
        self.packets.try_iter()
    }

    /// Encodes what's queued and flushes the encoder, handing back every packet left.
    pub(crate) fn finish(&mut self) -> impl Iterator<Item = Result<Packet, MediaError>> + '_ {
        self.samples = None;
        self.packets.iter()
    }

    /// Why the encoding thread stopped early.
    fn failure(&self) -> MediaError {
        self.packets
            .iter()
            .find_map(Result::err)
            .unwrap_or_else(|| MediaError::Thread("audio encoding stopped".into()))
    }
}

impl Drop for AudioEncoding {
    fn drop(&mut self) {
        self.samples = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl AudioStream {
    fn run(
        &mut self,
        queued: &Receiver<Vec<f32>>,
        packets: &Sender<Result<Packet, MediaError>>,
    ) -> Result<(), MediaError> {
        let whole = self.frame_size * STEREO;
        for stereo in queued {
            self.pending.extend_from_slice(&stereo);
            let mut offset = 0;
            while self.pending.len() - offset >= whole {
                self.encode(offset..offset + whole, packets)?;
                offset += whole;
            }
            self.pending.drain(..offset);
        }
        if !self.pending.is_empty() {
            self.encode(0..self.pending.len(), packets)?;
            self.pending.clear();
        }
        self.encoder.send_frame(None)?;
        self.drain(packets)
    }

    fn encode(
        &mut self,
        range: Range<usize>,
        packets: &Sender<Result<Packet, MediaError>>,
    ) -> Result<(), MediaError> {
        let samples = &self.pending[range];
        self.frame.fill_stereo(samples, self.frame_size)?;
        let frames = (samples.len() / STEREO) as i64;
        self.frame.set_timing(self.next_pts, frames);
        self.next_pts += frames;
        self.encoder.send_frame(Some(&self.frame))?;
        self.drain(packets)
    }

    fn drain(&mut self, packets: &Sender<Result<Packet, MediaError>>) -> Result<(), MediaError> {
        loop {
            let mut packet = Packet::new()?;
            match self.encoder.receive_packet(&mut packet)? {
                Received::Frame => {
                    // A closed channel means the writer gave up on the file.
                    if packets.send(Ok(packet)).is_err() {
                        return Ok(());
                    }
                }
                Received::NeedsInput | Received::Drained => return Ok(()),
            }
        }
    }
}
