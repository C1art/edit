//! Times decoding a file's video to RGBA through `MediaReader`, the path playback uses:
//! `cargo run --release -p media --example decode_speed -- <file>`.
use std::path::PathBuf;
use std::time::Instant;

use media::{DecodedFrame, MediaReader, Streams};
use timeline::Timebase;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path: PathBuf = std::env::args_os()
        .nth(1)
        .ok_or("usage: decode_speed <file>")?
        .into();
    let timebase = Timebase::new(1, 90_000).ok_or("bad timebase")?;
    let mut reader = MediaReader::open_streams(&path, timebase, Streams::Video)?;
    let started = Instant::now();
    let mut frames = 0u32;
    while let Some(frame) = reader.next_frame()? {
        if let DecodedFrame::Video(_) = frame {
            frames += 1;
        }
    }
    let elapsed = started.elapsed();
    println!(
        "{frames} frames in {elapsed:.2?}: {:.2} ms per frame",
        elapsed.as_secs_f64() * 1000.0 / f64::from(frames.max(1))
    );
    Ok(())
}
