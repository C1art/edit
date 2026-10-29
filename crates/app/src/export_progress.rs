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
}

impl ExportProgress {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            done: 0,
            total: 0,
        }
    }

    pub fn update(&mut self, done: i64, total: i64) {
        self.done = done;
        self.total = total;
    }

    pub fn bar(&self, width: f32) -> ProgressBar {
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
