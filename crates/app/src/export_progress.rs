use std::time::{Duration, Instant};

use egui::ProgressBar;

/// How often a running export's progress is redrawn; any faster only takes time
/// from the export.
pub const REFRESH: Duration = Duration::from_millis(250);

/// How far an export has got, timed from its start to tell how long it has left.
pub struct ExportProgress {
    started: Instant,
    done: i64,
    total: i64,
    encoder: Option<String>,
    /// How much of the sound has been measured, until the first frame is written.
    measuring: Option<f32>,
}

impl ExportProgress {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            done: 0,
            total: 0,
            encoder: None,
            measuring: None,
        }
    }

    pub fn update(&mut self, done: i64, total: i64) {
        if self.measuring.take().is_some() {
            // Measuring goes faster than writing, so its time would skew the estimate.
            self.started = Instant::now();
        }
        self.done = done;
        self.total = total;
    }

    pub fn measuring(&mut self, fraction: f32) {
        self.measuring = Some(fraction);
    }

    pub fn set_encoder(&mut self, encoder: String) {
        self.encoder = Some(encoder);
    }

    pub fn bar(&self, width: f32) -> ProgressBar {
        if let Some(fraction) = self.measuring {
            let text = format!("Measuring loudness  ·  {:.0}%", fraction * 100.0);
            return ProgressBar::new(fraction).desired_width(width).text(text);
        }
        let fraction = if self.total > 0 {
            self.done as f32 / self.total as f32
        } else {
            0.0
        };
        let elapsed = self.started.elapsed();
        let mut text = format!(
            "frame {} of {}  ·  {} elapsed",
            self.done,
            self.total,
            clock(elapsed)
        );
        if let Some(left) = remaining(elapsed, self.done, self.total) {
            text.push_str(&format!("  ·  {} left", clock(left)));
        }
        if let Some(encoder) = &self.encoder {
            text.push_str(&format!("  ·  {encoder}"));
        }
        ProgressBar::new(fraction).desired_width(width).text(text)
    }
}

/// The time left at the pace so far, once enough has been done to judge the pace.
fn remaining(elapsed: Duration, done: i64, total: i64) -> Option<Duration> {
    if done <= 0 || elapsed < Duration::from_secs(2) {
        return None;
    }
    let left = u32::try_from((total - done).max(0)).ok()?;
    let done = u32::try_from(done).ok()?;
    Some(elapsed / done * left)
}

fn clock(time: Duration) -> String {
    let seconds = time.as_secs();
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}
