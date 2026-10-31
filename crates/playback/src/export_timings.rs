use std::fmt;
use std::time::{Duration, Instant};

/// Where an export's time went. The render loop takes each frame's decoded layers from
/// the decoding thread, composites them, waits for the GPU's picture and mixes sound in
/// turn, handing each to the writer thread, which converts and encodes alongside. Time
/// the loop spends waiting on either thread means that thread holds the export back.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ExportTimings {
    pub frames: i64,
    pub total: Duration,
    /// Waiting for the decoding thread.
    pub decode: Duration,
    pub composite: Duration,
    pub gpu_wait: Duration,
    pub audio_mix: Duration,
    pub writer_wait: Duration,
    pub writer: WriterTimings,
}

/// What the writer thread spent its time on.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct WriterTimings {
    /// Turning pictures into the encoder's pixel format.
    pub convert: Duration,
    /// Encoding and muxing pictures, past converting them.
    pub encode_video: Duration,
    pub encode_audio: Duration,
    pub stills: Duration,
}

impl ExportTimings {
    /// Adds the time since `since` to `slot` and starts the next step's clock.
    pub(crate) fn lap(slot: &mut Duration, since: &mut Instant) {
        let now = Instant::now();
        *slot += now - *since;
        *since = now;
    }
}

impl fmt::Display for ExportTimings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let seconds = self.total.as_secs_f64().max(f64::EPSILON);
        let share = |part: Duration| 100.0 * part.as_secs_f64() / seconds;
        write!(
            f,
            "{} frames in {:.1} s ({:.1} fps). Render loop: waiting on decode {:.0}%, composite {:.0}%, \
             GPU wait {:.0}%, audio mix {:.0}%, waiting on writer {:.0}%. Writer: convert \
             {:.0}%, encode video {:.0}%, encode audio {:.0}%",
            self.frames,
            seconds,
            self.frames as f64 / seconds,
            share(self.decode),
            share(self.composite),
            share(self.gpu_wait),
            share(self.audio_mix),
            share(self.writer_wait),
            share(self.writer.convert),
            share(self.writer.encode_video),
            share(self.writer.encode_audio),
        )?;
        if !self.writer.stills.is_zero() {
            write!(f, ", stills {:.0}%", share(self.writer.stills))?;
        }
        Ok(())
    }
}
