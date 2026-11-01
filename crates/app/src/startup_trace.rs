//! With `CLEDIT_TRACE` naming a file, start-up writes each step there as it begins and
//! syncs it to disk, so the last line says how far a start that took the whole machine
//! down got.

use std::fmt::Display;
use std::fs::File;
use std::io::Write;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

const TRACED_FRAMES: u32 = 5;

struct Trace {
    file: Mutex<File>,
    began: Instant,
}

fn trace() -> Option<&'static Trace> {
    static TRACE: OnceLock<Option<Trace>> = OnceLock::new();
    TRACE
        .get_or_init(|| {
            let file = File::create(std::env::var_os("CLEDIT_TRACE")?).ok()?;
            Some(Trace {
                file: Mutex::new(file),
                began: Instant::now(),
            })
        })
        .as_ref()
}

pub(crate) fn step(what: impl Display) {
    let Some(trace) = trace() else { return };
    let Ok(mut file) = trace.file.lock() else {
        return;
    };
    let _ = writeln!(file, "{:>7} ms  {what}", trace.began.elapsed().as_millis());
    let _ = file.sync_all();
}

/// Only the first few frames, which is as far as start-up goes.
pub(crate) fn frame() {
    static FRAMES: AtomicU32 = AtomicU32::new(0);
    let frame = FRAMES.fetch_add(1, Ordering::Relaxed) + 1;
    if frame <= TRACED_FRAMES {
        step(format_args!("frame {frame}"));
    }
}
